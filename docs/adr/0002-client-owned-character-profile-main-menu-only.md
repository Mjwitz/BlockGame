# Client-Owned Character Profile Edited Only from Main Menu

A player's Character Appearance and saved Character Presets are stored locally in the client profile and transmitted to multiplayer servers during the join handshake (`AuthRequest`), with editing restricted exclusively to the Main Menu. This lets a player's identity follow them seamlessly across all singleplayer worlds and multiplayer servers while keeping server storage simple and avoiding mid-session appearance-sync complexity.

## Consequences

- Servers do not persist character appearances; they validate, clamp, and broadcast the appearance supplied at join for the duration of the session.
- Changing appearance during a multiplayer session requires returning to the Main Menu and rejoining.
