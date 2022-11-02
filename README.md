# aster-server

Aster is a distributed chat protocol, designed to be a middle ground between services like Discord and older protocols like IRC. Servers are self-hosted therefore your data is your own and server owners can have full control over their servers, including modifying the code. However, this does come with some downsides, especially in the realms of convenience: Server owners have to host their own servers on their own computers, which is potentially costly and difficult for non-programmers; and users must know the IP of a server to connect to it. Furthermore, impersonation is fairly easy as there is no inter-server communication, thus accounts are not unique to the user but rather to the server. Some of these problems can be improved client-side: nicknames, passwords and profile pictures are all updated in all servers at once, and a single server is chosen to be a "sync server" to store such things as current username and list of joined servers across multiple devices.

This repository is for an implementaiton of the Aster server. For the client, see the [web client](https://github.com/Jachdich/aster-web) and the [deprecated desktop GUI](https://github.com/Jachdich/aster-experimental-gui) (soon to be rewritten).

# TODO

- [x] Basic text messaging support
- [x] Multiple channels
- [x] Create and log in to accounts
- [x] Modify accounts (nick, pfp etc)
- [x] Encrypt connections (Currently using TLS, may switch to a different protocol in future)
- [ ] Properly hash and store passwords
- [ ] Permissions system
- [ ] Image support
- [ ] Store which messages have been read by which user
- [ ] List of online accounts & statuses for those accounts (partially implemented)
- [ ] Voice communication (far future)
