# Aster protocol
All packets are in JSON format, and have at least a single `command` field. Clientbound packets often have a `status` field, utilising HTTP status codes to indicate success or failure.
```json
{
    "command": "command_name",
    ...
}
```
 
## Serverbound

### Nick
Change the username of the currently logged in user. Must be logged in.

Command name: `nick`

Fields:
- `nick`: [str] the new nickname to be applied

### Register
Register a new user with given username and password. Must be logged out. On successful user creation, the user will automatically be logged in.

Command name: `register`

Fields:
- `username`: Initial username to assign to new user
- `password`: Initial password to assign to new user

## Clientbound

### Nick
Response to serverbound `nick` command.
Fields:
- `status`: either `403` if user is not logged in, or `400` if fields are missing. Otherwise, `200`
