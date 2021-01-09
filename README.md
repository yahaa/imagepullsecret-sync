# imagepullsecret-sync
Using kubernetes rust client [kube-rs](https://github.com/clux/kube-rs) to write an imagepullsecret sync worker.

### Create secret
```bash
k -n default create secret generic docker-registry --from-file=registry_secrets=registry_secrets.yaml --dry-run -o yaml | kubectl apply -f -
```
### Working Mechanism
* Pares above secret(docker-registry), create a secret(the same as `kubectl create secret docker-registry xxx`) for every specify namespace.
* Patch the above secret name to the serviceaccount imagePullSecrets field.
* [more detail](https://kubernetes.io/docs/concepts/containers/images/) 

### Build
```
$ docker build -t imagepullsecret-sync .
```