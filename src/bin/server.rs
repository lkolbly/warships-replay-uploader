use warp::Filter;

use sha2::Digest;
use sha2::Sha256;
use std::io::Write;
use std::path::Path;

fn get_paths(hash: &str) -> (String, String) {
    let first_dir = &hash[0..2];
    let second_dir = &hash[0..6];

    let dir_path = format!("data/{}/{}", first_dir, second_dir);
    let path = format!("data/{}/{}/{}", first_dir, second_dir, hash);
    (dir_path, path)
}

fn insert_data(data: &[u8]) -> std::io::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = hex::encode(hasher.finalize());

    let (dir_path, file_path) = get_paths(&hash);
    std::fs::create_dir_all(&dir_path)?;

    let mut f = std::fs::File::create(&file_path)?;
    f.write_all(data)?;
    Ok(())
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    // GET /hello
    let hello = warp::get().and(warp::path("hello")).map(|| "Hello, world!");

    // POST /contains  ["hashA", "hashB", "hashC", ...]
    let contains = warp::post()
        .and(warp::path("contains"))
        // Only accept bodies smaller than 1MB
        .and(warp::body::content_length_limit(1024 * 1024))
        .and(warp::body::json())
        .map(|hashes: Vec<String>| {
            //employee.rate = rate;
            //warp::reply::json(&employee)
            let mut present = vec![];
            for hash in hashes.iter() {
                // Sanitize the hash to contain only 0-9a-f
                let hash: String = hash
                    .to_lowercase()
                    .chars()
                    .filter(|c| {
                        "0123456789abcdef"
                            .chars()
                            .filter(|valid_c| c == valid_c)
                            .count()
                            > 0
                    })
                    .collect();

                let (_, file_path) = get_paths(&hash);
                if Path::new(&file_path).exists() {
                    present.push(true);
                } else {
                    present.push(false);
                }
            }
            warp::reply::json(&present)
        });

    // POST /insert <binary blob>
    let insert = warp::post()
        .and(warp::path("insert"))
        // Only accept bodies smaller than 50MB
        .and(warp::body::content_length_limit(1024 * 1024 * 50))
        .and(warp::body::bytes())
        .map(|data: bytes::Bytes| match insert_data(&data) {
            Ok(_) => "OK",
            Err(_) => "ERR",
        });

    let api = contains.or(insert).or(hello);

    warp::serve(api.with(warp::log("server")))
        .run(([0, 0, 0, 0], 3030))
        .await
}
