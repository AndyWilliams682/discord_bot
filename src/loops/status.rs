use std::{sync::Arc, time::Duration};

use rand::{distributions::WeightedIndex, rngs::StdRng, SeedableRng, prelude::Distribution};
use serenity::{prelude::Context, gateway::ActivityData};

const STATUS_UPDATE_TIMER_SECS: u64 = 3600;

pub fn start(ctx: Arc<Context>) {
    tokio::spawn(async move {
        loop {
            set_status(Arc::clone(&ctx)).await;
            tokio::time::sleep(Duration::from_secs(STATUS_UPDATE_TIMER_SECS)).await;
        }
    });
}

async fn set_status(ctx: Arc<Context>) {
    let status_options = [
        "a Plains",
        "an Island",
        "a Swamp",
        "a Mountain",
        "a Forest",
        "a Wastes",
        "a... Water Energy??"
    ];

    let status_weights = [17, 17, 17, 17, 17, 10, 5];

    let dist = WeightedIndex::new(&status_weights).unwrap();
    let mut rng = StdRng::from_entropy();

    let status_choice = status_options[dist.sample(&mut rng)];

    ctx.set_activity(Some(ActivityData::playing(status_choice)));
}