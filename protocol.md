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

### `register`
Register a new user with given username and password. Must be logged out. On successful user creation, the user will automatically be logged in. See clientbound [`register`](#register-1) packet.

Fields:
- `name`: [str] Initial username to assign to new user
- `passwd`: [str] Initial password to assign to new user

---

### `login`
Log in user, either using username or uuid to identify the user. Either the `ima,e` field or the `uuid` field must be present, not both (or neither). See clientbound [`login`](#login-1) packet.

Fields:
- `passwd`: [str] User's password
- `uname`: [str] (optional) Username to log into. Must be present if `uuid` is not.
- `uuid`: [int] (optional) UUID to log into. Must be present if `uname` is not.

---

### `nick`
Change the username of the currently logged in user. Must be logged in.

Fields:
- `nick`: [str] the new nickname to be applied

---

### `online`
Return a list of online UUIDs. See clientbound [`online`](#online-1) packet.

Fields: None

---

### `ping`
Essentially do nothing, and send a `200` status code response.

Fields: None

---

## Clientbound

### `register`
Response to serverbound [`register`](#register) command.

Fields:
- `status`: `405` if user is already logged in
- `uuid`: [int] The UUID of the newly created user

---

### `register`
Response to serverbound [`login`](#login) command.

Fields:
- `status`: `405` if user is already logged in
- `uuid`: [int] The UUID of the logged in user

---

### `nick`
Response to serverbound [`nick`](#nick) command.

Fields:
- `status`: `403` if user is not logged in

---

### `online`
Return a list of online UUIDs. May be sent as a response to serverbound [`online`](#online) packet, or in response to a user joining or leaving.

Fields:
- `data`: List[int] List of UUIDs
- `status`: `403` if user is not logged in

---

### `ping`
Response to serverbound [`ping`](#ping) command.

Fields:
- `status`: always `200`

---
