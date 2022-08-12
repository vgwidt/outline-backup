mod error;

use reqwest::Client;
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Cursor;
use std::io::{Read, Write};

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    server: String,
    secure: bool,
    apikey: String,
    timeout: u64,
    location: String,
}

const CONFIG_FILE_NAME: &str = "config.toml";

#[tokio::main]
async fn main() {
    let config = get_config();

    let mut response = build_post_request("collections.export_all", &config).send();
    let mut response_text = response.await.unwrap().text().await.unwrap();
    let mut response_body_json: Value = serde_json::from_str(&response_text).unwrap();

    //print formatted json
    println!(
        "{}",
        serde_json::to_string_pretty(&response_body_json).unwrap()
    );

    error::validate_response(
        &response_body_json["status"].to_string(),
        &response_body_json["error"].to_string(),
    );

    //get id from "data:" { "fileOperation": "id" }
    let id = response_body_json["data"]["fileOperation"]["id"]
        .as_str()
        .unwrap();

    //body
    let body = json!({
         "id": id,
    });

    // f there is an rate limit error, we cannot see it and the loop will continue indefinitely
    //So, stop if it exceeds timeout
    let mut status: String = String::new();
    let mut timer = 0;
    while status != "complete" && timer < config.timeout {
        response = build_post_request("fileOperations.info", &config)
            .body(body.to_string())
            .send();
        response_text = response.await.unwrap().text().await.unwrap();
        response_body_json = serde_json::from_str(&response_text).unwrap();
        println!("{}", response_body_json);
        status = response_body_json["data"]["state"]
            .as_str()
            .unwrap()
            .to_string();
        println!("{}", status);

        std::thread::sleep(std::time::Duration::from_millis(1000));
        timer += 1;
    }

    if timer >= config.timeout {
        println!("Timeout exceeded");
        std::process::exit(1);
    }

    let res = build_post_request("fileOperations.redirect", &config)
        .body(body.to_string())
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();

    //If length is less than 900, it is likely an error (very crude)
    if res.len() < 900 {
        //convert res from bytes to text then check if it contains "<Error><Code>InvalidRequest</Code><Message>
        let res_text = String::from_utf8(res.to_vec()).unwrap();
        if res_text.contains("<Error><Code>InvalidRequest</Code>") {
            println!("{}", res_text);
            //This uses some magic to actually download the file. Outline generates Jwt auth token in URL, but the redirect to Minio fails
            //because of too many authentication methods (works fine in Curl?) So we just use a get request with the Jwt token in the URL,
            //which we can get by inducing the error from Minio.

            //extract url from response between <Resource> and </Resource> from the Minio error
            let url = res_text
                .split("<Resource>")
                .nth(1)
                .unwrap()
                .split("</Resource>")
                .nth(0)
                .unwrap();
            let link = config.server.to_string() + &url;
            println!("{}", link);
            //download file from link and write to test.zip
            let mut file = std::fs::File::create("outline-backup.zip").unwrap();
            let client = Client::new();
            let response_text = client
                .get(link)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap();
            let mut cursor = Cursor::new(response_text);

            std::io::copy(&mut cursor, &mut file).unwrap();
        } else {
            println!("Another error occured");
        }
    } else {
        //Download regularly
        let mut file = std::fs::File::create("outline-backup.zip").unwrap();
        let mut cursor = Cursor::new(res);
        std::io::copy(&mut cursor, &mut file).unwrap();
    }

    move_backup(&config.location);

    //delete the backup from outline
    response = build_post_request("fileOperations.delete", &config)
        .body(body.to_string())
        .send();
    response_text = response.await.unwrap().text().await.unwrap();
    let deleteResponseBodyJson: Value = serde_json::from_str(&response_text).unwrap();
    println!("{}", deleteResponseBodyJson);
}

fn build_post_request(apicall: &str, config: &Config) -> reqwest::RequestBuilder {
    let mut post = reqwest::Client::new().post(config.server.to_owned() + "/api/" + apicall);
    post = post.header("Content-Type", "application/json");
    post = post.header("authorization", "Bearer ".to_owned() + &config.apikey);
    post = post.header("Accept", "application/json");

    return post;
}

fn get_config() -> Config {
    //set config file folder
    
    let mut config_file = String::new();
    if cfg!(windows) {
        //create Outline folder in APPDATA if it doesnt exist
        let mut path = std::env::var("APPDATA").unwrap();
        path.push_str("\\outline-backup\\");
        if !std::path::Path::new(&path).exists() {
            std::fs::create_dir_all(&path).unwrap();
        }
        config_file = path + CONFIG_FILE_NAME;
    } else {
        let mut path = std::env::var("HOME").unwrap();
        path.push_str("/.config/outline-backup/");
        if !std::path::Path::new(&path).exists() {
            std::fs::create_dir_all(&path).unwrap();
        }
        config_file = path + CONFIG_FILE_NAME;
    }

    println!("{}", config_file);

    if !std::path::Path::new(&config_file).exists() {
        match create_settings_file(&config_file) {
            Ok(_) => {}
            Err(e) => {
                println!("Failed to create config file{}", e);
                std::process::exit(1);
            }
        }
    }

    let mut file = std::fs::File::open(&config_file).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let mut config: Config = toml::from_str(&contents).unwrap();

    if config.secure {
        config.server = format!("https://{}", config.server);
    } else {
        config.server = format!("http://{}", config.server);
    }

    return config;
}

fn move_backup(fileloc: &String) {
    let time = chrono::Local::now();
    let time_str = time.format("%Y-%m-%d-%H-%M-%S").to_string();

    //Verify config.location path exists - if "" it should determine path doesn't exist but might break in some cases (testing needed)
    let path = std::path::Path::new(&fileloc);
    if !path.exists() {
        println!("Path does not exist or not specified, renaming only");
        //just rename it
        std::fs::rename(
            "outline-backup.zip",
            "outline-backup-".to_owned() + &time_str + ".zip",
        )
        .unwrap();
    } else {
        let time = chrono::Local::now();
        let time_str = time.format("%Y-%m-%d-%H-%M-%S").to_string();

        //absolute meaning C:\ or starts with /
        if path.is_absolute() {
            std::fs::copy(
                "outline-backup.zip",
                &path.join("outline-backup-".to_owned() + &time_str + ".zip"),
            )
            .unwrap();
            std::fs::remove_file("outline-backup.zip").unwrap();
        } else {
            std::fs::rename(
                "outline-backup.zip",
                &path.join("outline-backup-".to_owned() + &time_str + ".zip"),
            )
            .unwrap();
        }
    }
}

fn create_settings_file(config_file: &String) -> Result<(), std::io::Error> {
    //prompt for settings
    println!("Settings file not found, please enter settings");
    println!("Server (Include port in server if necessary (e.g. 192.168.1.200:8080))");
    let mut server = String::new();
    std::io::stdin().read_line(&mut server).unwrap();
    let server = server.trim().to_string();

    let mut secure: bool = false;
    let mut secure_yn = String::new();
    loop {
        println!("HTTPS? (y/n)");
        std::io::stdin().read_line(&mut secure_yn).unwrap();
        let secure_yn = secure_yn.trim().to_string();
        if secure_yn == "y" || secure_yn == "n" || secure_yn == "Y" || secure_yn == "N" {
            break;
        } else {
            println!("Invalid input");
        }
    }
    if secure_yn == "y" {
        secure = true;
    } else {
        secure = false;
    }

    println!("API Key");
    let mut apikey = String::new();
    std::io::stdin().read_line(&mut apikey).unwrap();
    let apikey = apikey.trim().to_string();
    println!("Location to store backup (e.g. C:\\Users\\User\\Desktop or /home/user/Desktop)");
    let mut location = String::new();
    std::io::stdin().read_line(&mut location).unwrap();
    //double-up backslash (escape chars)
    let location = location.replace("\\", "\\\\");
    let location = location.trim().to_string();

    println!(
        "Set request timeout (in seconds, recommend at least 60 seconds for small organizations)"
    );
    let mut timeout = String::new();
    loop {
        std::io::stdin().read_line(&mut timeout).unwrap();
        timeout = timeout.trim().to_string();
        if timeout.parse::<u64>().is_ok() {
            break;
        } else {
            println!("Invalid input, please enter a number");
        }
    }
    let timeout = timeout.parse::<u64>().unwrap();

    let config = Config {
        server,
        apikey,
        location,
        secure,
        timeout,
    };

    let config_str = toml::to_string(&config).unwrap();

    //try to create file
    match std::fs::write(config_file, &config_str) {
        Ok(_) => {
            return Ok(());
        }
        Err(e) => match std::fs::write(CONFIG_FILE_NAME, config_str) {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                return Err(e);
            }
        },
    }
}
