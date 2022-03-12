# Webhook-Server

 [![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

This little helper serves a simple purpose: Execute commands on your server on incoming http requests.
It has been designed for continuous integration and supports Github's webhooks out of the box.

Webhook-Server works in conjunction with [Pueue](https://github.com/nukesor/pueue), which allows easy output inspection, loggin and debugging of your webhook calls.

**Example applications:**

- Continuous integration for projects (Supports Github's webhooks).
- On-Demand execution of parallel load-heavy tasks.
- Trigger tasks on your server via a browser.
- Trigger tasks between servers with a minimal setup.

Take a look at the example config file [webhook_server.yml](https://github.com/Nukesor/pueue-webhook-server/blob/master/webhook_server.yml).

## Installation

**Manual installation:**

```bash
git clone https://github.com/nukesor/webhook-server
cd webhook-server
cargo install --path .
```

Your `$CARGO_HOME/bin` folder should be in your $PATH.

## Configuration

Webhook-Server is configured via files in this order:

- `/etc/webhook_server.yml`
- `~/.config/webhook_server.yml`
- `./webhook_server.yml`

Config values of higher hierarchy config files are overwritten by lower hierarchy config files. E.g. a value in `/etc/webhook_server.yml` can be overwritten by `~/.config/webhook_server.yml`.

### Config values

- `domain (127.0.0.1)` The domain the server should listen on
- `port (8000)` The port the server should listen on
- `secret (null)` A secret for authentication via payload signature verification. Check the `Building a request` section for more information on signature headers. Can be, for instance, be created with `pwgen 25 1`
- `ssl_private_key (null)` Path to SSL private key. The server will use it's own ssl certificate. Recommended, if you aren't using a proxy webserver, that already uses SSL. Using any kind of SSL is highly recommended, especially if you publicly expose your endpoint.
- `ssl_cert_chain (null)` Path to SSL cert. Also required for SSL setup.
- `basic_auth_user (null)` Your user if you want to do basic auth. Check the `Building a request` section for more information on basic_auth headers
- `basic_auth_password (null)` Your password if you want to do basic auth.
- `basic_auth_and_secret (false)` By default it's only required to authenticate via BasicAuth OR signature authentication. If you want to be super safe, set this to true to require both.
- `pueue_port (6924)` Set this to the port your local pueue instance listens on.
- `pueue_unix_socket (null)` In case you're using unix sockets, set this to your Pueue's socket path and `pueue_port` to `null`.
- `pueue_directory` The working directory of Pueue, can be found in Pueue's configuration file.
- `webhooks` A list of webhooks. The whole thing looks pretty much like this:

```yaml
webhooks:
  -
    name: 'ls'
    command: '/bin/ls {{param1}} {{param2}}'
    cwd: '/home/user'
    pueue_group: 'webhook'
```

**Webhook config values**

- `name` The name of the webhook, also the endpoint that's used to trigger the webhooks. E.g. `localhost:8000/ls`.
- `command` The command thats actually used. If you want to dynamically build the command, you can use templating parameters like `{{name_of_parameter}}`.
- `cwd` The current working directory the command should be executed from.
- `pueue_group` Which pueue group should be used for this webhook.

## Misc files

There are some template files for your setup in the [misc folder](https://github.com/Nukesor/pueue-webhook-server/tree/master/misc) of the repository.
These include:

- A nginx proxy route example
- A systemd service file

If you got anything else that might be useful to others, feel free to create a PR.

## Github Webhook Setup

Go to your project's settings tab and select webhooks. Create a new one and set these options:

- Content-Type: Json
- Secret: Same string as in your config
- Enable SSL verification: Recommended, if you have any kind of SSL
- Just the push event (The payload isn't used anyway)

You can click on the `Recent Deliveries` to redeliver any sent webhook, in case you want to debug your setup.

## Building a request

Webhook server accepts JSON POST requests and simple GET requests.

This is an example POST request issued with `httpie` and a secret of `72558847d57c22a2f19d711537cdc446` and `test:testtest` basic auth credentials:

```bash
echo -n '{"parameters":{"param1":"-al","param2":"/tmp"}}' | http POST localhost:8000/ls \
        Signature:'sha1=d762407ca7fb309dfbeb73c080caf6394751f0a4' \
        Authorization:'Basic dGVzdDp0ZXN0dGVzdA=='
```

If you don't need templating, you can send a simple GET request:

```bash
http GET localhost:8000/ls Authorization:'Basic dGVzdDp0ZXN0dGVzdA=='
```

**Payload:**

The payload is a simple JSON object, with a single entry `parameters`.
This object contains all parameters necessary for rendering the template.
If no templating is needed, you can provide an empty object as payload or simply call the route via `GET`.

For instance, the payload for the command `'/bin/ls {{param1}} {{param2}}'` could look like this:

```json
{
    "parameters": {
        "param1": "-al",
        "param2": "/tmp"
    }
}
```

This would result in the execution of `ls -al /tmp` by the server.

**Headers:**

- `Authorization`: If `basic_auth_username` and `basic_auth_password` is specified, this should be the standard `Basic` base64 encoded authorization header. [Basic Auth guide](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Authorization)
- `Signature:` If you specify a secret, the content of the signature is the HMAC of the json payload with the UTF8-encoded secret as key.
    This procedure is based on Github's webhook secret system. (Github tells you to use a hex key, but they interpret it as UTF8 themselves -.-)  
    Python example: `hmac.new(key, payload, hashlib.sha1)`  
    Ruby example: `OpenSSL::HMAC.hexdigest("SHA1", key, payload)`  
    [Github guide](https://developer.github.com/webhooks/securing/)
- `X-Hub-Signature`: If there is no `Signature`, this header will be used for the signature check (to support Github's webhooks).

## Security

**Code injection:**
When compiling dynamic commands with templating, you make yourself vulnerable to code injection, since the compiled commands are executed by the system shell.
If you plan on using templating and publicly exposing your service, please use some kind of authentication.

1. You can use a secret to verify the payload with a signature (Github's authentication method). Anyway, this method is a bit annoying to implement, if you write your own implementation.
2. You can use basic auth.
3. If you want to be super safe, you can require both authentication methods.

**SSL:**
Especially when using Basic Auth or templating it's highly recommended to use SSL encryption.
This can be either done by your proxy web server (nginx, apache, caddy) or directly in the application.
Otherwise your credentials or your template payload could leak to anybody listening.

An example cert and key can be created like this `openssl req -nodes -new -x509 -keyout test.pem -out test.pem`.  
If you need a password input for the private key, please create an issue or PR (much appreciated).
