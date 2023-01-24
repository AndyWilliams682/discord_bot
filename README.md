# discord_bot
A project for learning Rust and creating a bot for Discord with the Serenity crate.



# Docker Build
To build the docker image run:
```
docker build -t discord_bot .
```

To run the docker image run:
```
docker run -it --rm --env DISCORD_TOKEN=<TOKEN> --env GUILD_ID=<GUID> --name discord_bot discord_bot 
```
