use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub server: String,
    pub username: String,
    pub password: String,
    // if namespaces.len()==0 || namespaces.contains("*")
    // then this config effect all namespace, else this config
    // only effect on the specify namespaces.
    pub namespaces: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryAuth {
    auths: BTreeMap<String, UserInfo>,
}

impl RegistryAuth {
    pub fn new(username: String, password: String, server: String) -> Self {
        let mut auths = BTreeMap::new();
        auths.insert(server, UserInfo::new(username, password));
        RegistryAuth { auths }
    }
    pub fn base64_encode(&self) -> String {
        base64::encode(serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserInfo {
    username: String,
    password: String,
    auth: String,
}

impl UserInfo {
    fn new(username: String, password: String) -> Self {
        let auth = base64::encode(format!("{}:{}", username, password));
        UserInfo {
            username,
            password,
            auth,
        }
    }
}

#[cfg(test)]
mod test {
    use super::RegistryAuth;

    #[test]
    fn base64_encode() {
        let ra = RegistryAuth::new(
            "yuanzihua".to_string(),
            "123456".to_string(),
            "registry.zihua.com".to_string(),
        );

        let want="eyJhdXRocyI6eyJyZWdpc3RyeS56aWh1YS5jb20iOnsidXNlcm5hbWUiOiJ5dWFuemlodWEiLCJwYXNzd29yZCI6IjEyMzQ1NiIsImF1dGgiOiJlWFZoYm5wcGFIVmhPakV5TXpRMU5nPT0ifX19";
        let got = ra.base64_encode();
        assert_eq!(want, got);
    }
}
