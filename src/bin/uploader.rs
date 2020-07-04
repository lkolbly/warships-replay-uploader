use std::env;
use warships_replay_poller::upload_replays;

fn main() {
    pretty_env_logger::init();

    let cli_args: Vec<String> = env::args().collect();
    if cli_args.len() != 3 {
        println!(
            "Usage: {} <World of Warships install path> http://pillow.rscheme.org/wows-replays",
            cli_args[0]
        );
        return;
    }

    upload_replays(&cli_args[1], &cli_args[2], std::collections::HashSet::new()).unwrap();
}
