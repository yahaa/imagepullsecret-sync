use anyhow::{anyhow, Result};
use futures::{StreamExt, TryStreamExt};
use imagepullsecret_sync::config::{Config, RegistryAuth};
use k8s_openapi::api::core::v1::{LocalObjectReference, Namespace, Secret, ServiceAccount};
use kube::{
    api::{ListParams, Meta, PatchParams, PostParams},
    Api, Client,
};
use kube_runtime::watcher;

use serde_json::json;
use watcher::Error::WatchError;
use watcher::Event::{Applied, Restarted};

#[derive(Clone)]
pub struct SyncWorker<'a> {
    cfg_ns: &'a str,
    cfg_name: &'a str,
    cfg_data_key: &'a str,
    sa_name: &'a str,
    client: Client,
}

impl<'a> SyncWorker<'a> {
    pub fn new(client: Client, cfg_ns: &'a str, cfg_name: &'a str) -> Self {
        SyncWorker {
            client,
            cfg_ns,
            cfg_name,
            cfg_data_key: "registry_secrets", // todo
            sa_name: "default",               // todo
        }
    }

    async fn ensure(&self, all_ns: Vec<String>, configs: Vec<Config>) {
        for ns in all_ns.iter() {
            for cfg in configs.iter() {
                match self.ensure_registry_secret(ns, cfg).await {
                    Ok(skip) => {
                        if !skip {
                            if let Err(e) = self.ensure_patch_sa(ns, &cfg.server).await {
                                warn!("patch '{}/{}' to default err: {}", ns, cfg.server, e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("ensure '{}/{}' registry_secret err: {}", ns, cfg.server, e);
                    }
                }
            }
        }
    }

    pub async fn watch_ns(&self) -> Result<()> {
        info!("watching all active ns ...");
        let ns_api = Api::<Namespace>::all(self.client.clone());

        let lp = ListParams::default().fields("status.phase=Active");

        let mut w = watcher(ns_api, lp).boxed();

        loop {
            match w.try_next().await {
                Ok(res) => {
                    if let Some(event) = res {
                        match event {
                            Applied(ns) => match self.read_config().await {
                                Ok(configs) => {
                                    let all_ns = Vec::from(vec![ns.name()]);
                                    self.ensure(all_ns, configs).await;
                                    info!("ns {} applied, ensure for it successful.", ns.name());
                                }
                                Err(e) => {
                                    error!("applied ns {}, but read_config err: {}", ns.name(), e)
                                }
                            },
                            Restarted(nss) => {
                                // if read config err stop watch
                                let configs = self.read_config().await?;
                                let all_ns = nss.iter().map(|ns| ns.name()).collect();

                                self.ensure(all_ns, configs).await;

                                info!("restarted, ensure for {} ns successful.", nss.len());
                            }
                            _ => {}
                        }
                    }
                }
                Err(WatchError { source: e, .. }) => {
                    warn!("watch all ns err: {}, retrying...", e)
                }
                Err(e) => return Err(anyhow!("got err: {} exist watcher.", e)),
            }
        }
    }

    pub async fn watch_cfg_secret(&self) -> Result<()> {
        info!("watching secret '{}/{}' ...", self.cfg_ns, self.cfg_name);
        let secret_api = Api::<Secret>::namespaced(self.client.clone(), self.cfg_ns);

        let lp = ListParams::default().fields(&format!("metadata.name={}", self.cfg_name));

        let mut w = watcher(secret_api, lp).boxed();

        loop {
            match w.try_next().await {
                Ok(res) => {
                    if let Some(event) = res {
                        match event {
                            Applied(s) => match self.read_data(s).await {
                                Ok(configs) => match self.get_all_ns().await {
                                    Ok(all_ns) => self.ensure(all_ns, configs).await,
                                    Err(e) => error!("get all ns err: {}", e),
                                },
                                Err(e) => {
                                    error!("applied {} cfg, read_data err: {}", self.cfg_name, e)
                                }
                            },
                            _ => {}
                        }
                    }
                }
                Err(WatchError { source: e, .. }) => {
                    warn!("watch all ns err: {}, retrying...", e)
                }
                Err(e) => return Err(anyhow!("got err: {} exist watcher.", e)),
            }
        }
    }

    async fn get_all_ns(&self) -> Result<Vec<String>> {
        let ns_api = Api::<Namespace>::all(self.client.clone());
        let lp = ListParams::default().fields("status.phase=Active");

        let all_ns = ns_api.list(&lp).await?;

        Ok(all_ns.items.iter().map(|item| item.name()).collect())
    }

    async fn ensure_registry_secret(&self, ns: &str, cfg: &Config) -> Result<bool> {
        if !cfg.namespaces.contains(&format!("*")) && !cfg.namespaces.contains(&format!("{}", ns)) {
            debug!("secret '{}' don't need sync to ns '{}'", cfg.server, ns);
            return Ok(true);
        }

        debug!("registry secret '{}'/'{}' applying...", ns, cfg.server);

        let auth = RegistryAuth::new(
            cfg.username.clone(),
            cfg.password.clone(),
            cfg.server.clone(),
        );

        let key = ".dockerconfigjson";

        let secret_api = Api::<Secret>::namespaced(self.client.clone(), self.cfg_ns);
        match secret_api.get(&cfg.server).await {
            Ok(s) => {
                if let Some(map) = s.data {
                    if let Some(data) = map.get(key) {
                        if base64::encode(&data.0) != auth.base64_encode() {
                            let js = json!({ "data": {key: auth.base64_encode() } });
                            let p = serde_json::to_vec(&js)?;
                            let pp = PatchParams::default();
                            secret_api.patch(&cfg.server, &pp, p).await?;
                        }
                    } else {
                        warn!("not found {} in map", key);
                    }
                } else {
                    warn!("secret's data field is None");
                }
            }
            Err(kube::Error::Api(e)) => {
                if e.code == 404 {
                    let s: Secret = serde_json::from_value(json!({
                            "apiVersion": "v1",
                            "data": {
                                ".dockerconfigjson": auth.base64_encode(),
                            },
                            "kind": "Secret",
                            "metadata": {
                                "name": cfg.server,
                                "namespace": ns,
                            },
                            "type": "kubernetes.io/dockerconfigjson"
                        }
                    ))?;

                    let pp = PostParams::default();

                    secret_api.create(&pp, &s).await?;
                }
            }
            Err(e) => return Err(anyhow!("query {} err: {}", cfg.server, e)),
        }

        Ok(false)
    }

    async fn ensure_patch_sa(&self, ns: &str, secret_name: &str) -> Result<()> {
        debug!("default sa '{}/{}' patching...", ns, secret_name);

        let sa_api = Api::<ServiceAccount>::namespaced(self.client.clone(), ns);

        let mut found = false;
        let mut new_secrets: Vec<LocalObjectReference> = Vec::new();

        match sa_api.get(self.sa_name).await {
            Ok(sa) => {
                if let Some(ipss) = sa.image_pull_secrets {
                    for item in ipss {
                        if item.name == Some(String::from(secret_name)) {
                            found = true
                        }
                        new_secrets.push(item);
                    }
                }
            }
            Err(e) => return Err(anyhow!("get {}/default sa err: {}", ns, e)),
        }

        if !found {
            let p = serde_json::to_vec(&json!({ "imagePullSecrets": new_secrets }))?;
            let pp = PatchParams::default();
            sa_api.patch(self.sa_name, &pp, p).await?;
        }

        Ok(())
    }

    async fn read_config(&self) -> Result<Vec<Config>> {
        let secret_api = Api::<Secret>::namespaced(self.client.clone(), self.cfg_ns);
        let secret = secret_api.get(self.cfg_name).await?;

        self.read_data(secret).await
    }

    async fn read_data(&self, secret: Secret) -> Result<Vec<Config>> {
        match secret.data {
            Some(map) => match map.get(self.cfg_data_key) {
                Some(byte_str) => {
                    let configs: Vec<Config> = serde_yaml::from_slice(&byte_str.0)?;
                    return Ok(configs);
                }
                None => Err(anyhow!("read secret data field {} err", self.cfg_data_key)),
            },
            None => Err(anyhow!("read secret data err")),
        }
    }
}
