# imagepullsecret-sync
Using kubernetes rust client [kube-rs](https://github.com/clux/kube-rs) to write an imagepullsecret sync tool.

### Create secret
```bash
k -n default create secret generic docker-registry-configs --from-file=registry_secrets=registry_secrets.yaml --dry-run -o yaml | kubectl apply -f -
```
### Working Mechanism
* Pares the secret above(docker-registry-configs),then create a docker-registry secret(the same as `kubectl create secret docker-registry xxx`) for each specify namespace.
* Patch the docker-registry secret name to the default serviceaccount imagePullSecrets field.
* [more detail](https://kubernetes.io/docs/concepts/containers/images/) 

### Build
```
$ docker build -t imagepullsecret-sync .
```
