use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::interaction::application_command::CommandDataOption;

pub fn run(_options: &[CommandDataOption]) -> String {
    "Hello secret Santa gamers, I have been informed that SOMEONE'S deck will arrive at their address within the week! HOWEVER, I have also been informed that the recipient should not open this package until a SECOND package arrives. HOW MYSTERIOUS".to_string()
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("secret").description("Shhh, no telling")
}