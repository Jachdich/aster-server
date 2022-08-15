# Aster protocol
All packets are in JSON format, and have at least a single `command` field. Clientbound packets often have a `status` field, utilising HTTP status codes to indicate success or failure.
```json
{
    "command": "command_name",
    ...
}
```

---

## Status Codes

Unless specified by the individual packet, the `status` field of a packet behaves in the following way:
- `200` to indicate success
- `400` if there is any problem with the packet e.g. invalid JSON or missing/wrong type fields
- `403` if the user is not logged in, and needs to be
- `405` if the command doesn't make sense in the current context e.g. `login` packet when already logged in

---

## Serverbound

### Nick
Change the username of the currently logged in user. Must be logged in.

Command name: `nick`

Fields:
- `nick`: [str] the new nickname to be applied

---

### Register
Register a new user with given username and password. Must be logged out. On successful user creation, the user will automatically be logged in.

Command name: `register`

Fields:
- `username`: Initial username to assign to new user
- `password`: Initial password to assign to new user

---

### Ping
Essentially do nothing, and send a `200` status code response.

Fields: None

---

### Online


---

## Clientbound

### Nick
Response to serverbound `nick` command.

Fields:
- `status`: `403` if user is not logged in

---

### Register
Response to serverbound `register` command.

Fields:
- `status`: `405` if user is logged in

---

### Ping
Response to serverbound `ping` command.

Fields:
- `status`: always `200`
