# aster-server

Aster is a distributed chat protocol, designed to be a middle ground between services like Discord and older protocols like IRC. Servers are self-hosted therefore your data is your own and server owners can have full control over their servers, including modifying the code.

This repository is for an implementaiton of the Aster server. For the client, see the [web client](https://github.com/Jachdich/aster-web) and the [CLI client](https://github.com/Jachdich/aster-cli).

# Hosting a server

To host an aster server you will need:
- A computer with a public IP address, or port forwarding set up
- A domain name and a valid TLS certificate (see [Getting certificates](#getting-certificates))

## Installing

Next, you'll need a binary of `aster-server`. It's currently recommended to clone this repository and build it with Cargo, which can be installed from [rust's website](https://www.rust-lang.org/). Someday I will compile releases (if you need one, bug me about it by opening an issue). Once rust is installed, you can build and run the server using the following commands:

```sh
git clone https://github.com/Jachdich/aster-server
cd aster-server
cargo build --release
# to run, you would use this command, but there are a few other things you need to do first
cargo run --release 
# or
./target/release/server
```
Next, you need to set up the database and the configuration file. If you cloned the repository with git, you'll already have the files, but if not go ahead and grab `init.sql` and `config.json` from the git repository.

## Configuration

There are four files the server needs to operate correctly:
- `config.json` - main configuration file
- `aster.db` - sqlite database file for storing all information
- `icon.png` - icon of the server, must be PNG format, consider a small size (<= 128x128) for fast loading times
- `default.png` - default profile picture of accounts without a defined profile picture. Consider an even smaller size, e.g. 32x32 as profile pictures are usually displayed very small.

All filenames ther than `config.json` can be nodified in the `config.json` file.

Inside `config.json` you will find the following structure:
```json
{
    "addr": "0.0.0.0",
    "port": 2345,
    "voice_port": 5432,
    "name": "Aster Server",
    "icon": "icon.png",
    "default_pfp": "default.png",
    "database_file": "aster.db"
}
```

- addr - the address to bind to (0.0.0.0 will bind to all addresses, 127.0.0.1 will only allow local clients to connect).
- port - i think you can work this one out
- voice_port - not currently used (maybe one day...)
- name - server name sent to connecting clients
- icon - filename of the server icon
- default_pfp - filename of the default user profile picture
- database_file - filename of the database file

## Setting up the database
TODO - someday I will make this automatic.

To set up the database you need to first install sqlite. Make sure you have the `init.sql` in your current directory. Run the following command to use it to make an empty database: (replace `aster.db` if you have modified it in the `config.json`)

```sh
cat init.sql | sqlite3 aster.db
```

## Updating the database
TODO: REALLY should make this automatic

If the schema of the database has changed in any way (likely to happen in early stages of development) then there will be a new file created with the extension `.sql` that should be used to modify the database. To use them pipe their contents into `sqlite3`:

```sh
cat file.sql | sqlite3 aster.db
```

## Getting certificates

I know, TLS certificates are a pain, but they are required for encryption on the web. A good free option to get TLS certificates is [Let's Encrypt](https://letsencrypt.org/). The easiest method is to use [certbot](https://certbot.eff.org/) to create and manage the certificates for you. You can find information on their website or [this tutorial](https://www.digitalocean.com/community/tutorials/how-to-use-certbot-standalone-mode-to-retrieve-let-s-encrypt-ssl-certificates-on-ubuntu-20-04) or [their official usage guide](https://eff-certbot.readthedocs.io/en/stable/using.html).

If you know what you're doing and don't want to use Let's Encrypt, the important thing is that you have:
- The private key (certbot calls this `privkey.pem`)
- The full certificate chain including the leaf/server/end-entity certificate (certbot calls this `fullchain.pem`)
These key files must be PEM-encoded.

Now, aster-server requires the two files called `privkey.pem` and `fullchain.pem` (TODO: filenames in the config) in its working directory. If you used certbot it is recommended to symlink your certificate files to allow them to be updated automatically, but you can also copy them into the server's directory.

## Running the server
Now hopefully everything should be properly set up and you can start the server. It is recommended to run the server using some tool like [GNU screen](https://wiki.archlinux.org/title/GNU_Screen) to keep tabs on the process. For example, if using `screen`:

```sh
screen -S <give it a name> cargo run --release
```

# Protocol information

The aster protocol is a fairly basic JSON protocol. It consists of a set of requests that are always replied to with the corrosponding response, and a set of responses that can be sent without a request first being made.

The general structure of a request is:

```json
{"command": "your_request", ...}
```

where `...` represents any other fields the command may have. For example:

```json
{"command": "send", "channel": 139513591350909, "content": "Hello, world!"}
```

The general structure of a response is:

```json
{"command": "your_request", "status": 200, ...}
```

where `...` represents any other fields the response may have, and 200 is an example status code (hopefully all of your requests will also result in a 200 status!).


## Datatypes

All images (profile pictures, server icons) are stored as base64-encoded PNG images.

Common formats returned by commands:

### SyncServer

```json
{
    uuid: Option<int>,
    uname: string,
    ip: string,
    port: int,
    pfp: Option<string>,
    name: Option<string>,
    idx: int,
}
```

### SyncData
```json
{
    user_uuid: int,
    uname: string,
    pfp: string,
}
```

### User
```json
{
    uuid: int,
    name: string,
    pfp: string,
    group_uuid: int,
}
```

### Emoji
```json
{
    uuid: int,
    name: string,
    data: string,
}
```

### Channel
```json
{
    uuid: int,
    name: string,
}
```

### Message
```json
{
    uuid: int,
    content: string,
    author_uuid: int,
    channel_uuid: int,
    date: int,
    edited: bool,
}
```


## List of requests
| Name             | Data                                                              |
| ---------------- | ----------------------------------------------------------------- |
| register         | passwd: string, uname: string                                     |
| login            | passwd: string, uname: Option\<string\>, uuid: Option\<int\>  |
| ping             |                                                                   |
| nick             | nick: string                                                      |
| online           |                                                                   |
| send             | content: string, channel: int                                 |
| get_metadata     |                                                                   |
| get_name         |                                                                   |
| get_icon         |                                                                   |
| list_emoji       |                                                                   |
| get_emoji        | uuid: int                                                     |
| list_channels    |                                                                   |
| history          | num: int, channel: int, before_message: Option\<int\> |
| pfp              | data: string                                                      |
| sync_set         | uname: string, pfp: string                                        |
| sync_get         |                                                                   |
| sync_set_servers | severs: list\[SyncServer\]                                        |
| sync_get_servers |                                                                   |
| leave            |                                                                   |
| get_user         | uuid: int                                                     |
| edit             | message: int, new_content: string                             |
| delete           | message: int                                                  |

## List of responses

If a response's status code is not 200, **no other fields will be present**.

| Name             | Data                                                     |
| ---------------- | -------------------------------------------------------- |
| register         | status: Status, uuid: int                        |
| login            | status: Status, uuid: int                        |
| get_metadata     | status: Status, data: list\[User\]                  |
| sync_get_servers | status: Status, servers: list\[SyncServer]         |
| online           | status: Status, data: list\[int\]                   |
| history          | status: Status, data: list\[Message\]               |
| get_user         | status: Status, data: User                       |
| get_icon         | status: Status, data: string                     |
| get_name         | status: Status, data: string                     |
| list_channels    | status: Status, data: list\[Channel\]               |
| get_emoji        | status: Status, data: Emoji                      |
| list_emoji       | status: Status, data: list\[\[string, int\]\]         |
| sync_get         | status: Status, user_uuid: int, uname: string, pfp: string |
| content          | status: Status, uuid: int, author_uuid: int, channel_uuid: int, content: string, date: int, edited: bool      |
| API_version      | status: Status, version: \[int, int, int\]                  |
| send             | status: Status, message: int,                            |
| edit             | status: Status                                           |
| delete           | status: Status                                           |
| message_edited   | status: Status, message: int, new_content: string        |
| message_deleted  | status: Status, message: int                             |


## Status codes
Aster uses a subset of HTTP status codes to encode success/failure of a 

| Code | Description      |
| ---- | ---------------- |
| 200  | Ok               |
| 400  | BadRequest       |
| 401  | Unauthorised     |
| 403  | Forbidden        |
| 404  | NotFound         |
| 405  | MethodNotAllowed |
| 409  | Conflict         |
| 500  | InternalError    |

## Description of fields
TODO: do this

# General information about aster
Aster's distributed nature does come with some downsides, which I have attempted to address. Especially in the realms of convenience: Server owners have to host their own servers on their own computers, which is potentially costly and difficult for non-programmers; and users must know the IP of a server to connect to it. Furthermore, impersonation is fairly easy as there is no inter-server communication, thus accounts are not unique to the user but rather to the server. Some of these problems can be improved client-side: nicknames, passwords and profile pictures are all updated in all servers at once, and a single server is chosen to be a "sync server" to store such things as current username and list of joined servers across multiple devices.

TODO: finish this + explain sync servers etc.

## Sync servers

# TODO

- [x] Basic text messaging support
- [x] Multiple channels
- [x] Create and log in to accounts
- [x] Modify accounts (nick, pfp etc)
- [x] Encrypt connections (Currently using TLS, may switch to a different protocol in future)
- [x] Properly hash and store passwords
- [x] edit and delete messages
- [ ] Permissions system
- [ ] Image support
- [ ] Store which messages have been read by which user
- [ ] List of online accounts & statuses for those accounts (partially implemented)
- [ ] Voice communication (far future)
- [ ] Autogenerate & update config.json and aster.db
- [ ] Autogenerate a cool pfp for new users
- [ ] ORGANISE MY BLOODY TODO LIST!