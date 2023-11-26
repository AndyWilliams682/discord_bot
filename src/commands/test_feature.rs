use serenity::builder::CreateApplicationCommand;
use serde_json::Value;

pub async fn run() -> String {
    let url = format!("https://api.scryfall.com/cards/search?order=color&q=ci%3Dcolorless+pow%3D3");
    let response = reqwest::get(&url).await.unwrap();
    let output = match response.status() {
        reqwest::StatusCode::OK => {
            match response.json::<Value>().await {
                Ok(parsed) => {
                    let abilities: &Vec<Value> = parsed["data"].as_array().unwrap();
                    let hidden_ability: String = abilities[0]["name"].as_str().unwrap().to_string();
                    hidden_ability
                }
                Err(why) => why.to_string()
            }
        }
        other => {
            format!("{}", other).to_string()
        }
    };
    // output
    "THIS IS A TEST".to_string()
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command.name("test_feature").description("Command for testing Scryfall API")
}