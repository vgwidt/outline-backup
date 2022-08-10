mod error;

use std::io::Read;
use serde_json::{Value, json};
use serde_derive::Deserialize;
use std::io::Cursor;
use reqwest::Client;

#[derive(Deserialize, Clone)]
struct Config {
    server: String,
    secure: bool,
    apikey: String,
    timeout: u64,
    location: String,
}

const CONFIGFILE: &str = "settings.toml";

#[tokio::main]
async fn main() {

    let config = get_config();

    let mut response = build_post_request("collections.export_all", &config).send();
    let mut response_text = response.await.unwrap().text().await.unwrap();
    let mut response_body_json: Value = serde_json::from_str(&response_text).unwrap();

    //print formatted json
    println!("{}", serde_json::to_string_pretty(&response_body_json).unwrap());

    error::validate_response(&response_body_json["status"].to_string(), &response_body_json["error"].to_string());

    //get id from "data:" { "fileOperation": "id" }
    let id = response_body_json["data"]["fileOperation"]["id"].as_str().unwrap();

    //body
    let body = json!({
         "id": id,
    });

    // f there is an rate limit error, we cannot see it and the loop will continue indefinitely
    //So, stop if it exceeds timeout
    let mut status:String = String::new();
    let mut timer = 0;
    while status != "complete" && timer < config.timeout {

         response = build_post_request("fileOperations.info", &config).body(body.to_string()).send();
         response_text = response.await.unwrap().text().await.unwrap();
         response_body_json = serde_json::from_str(&response_text).unwrap();
         println!("{}", response_body_json);
         status = response_body_json["data"]["state"].as_str().unwrap().to_string();
         println!("{}", status);

         std::thread::sleep(std::time::Duration::from_millis(1000));
         timer += 1;
     }

    if timer >= config.timeout {
        println!("Timeout exceeded");
        std::process::exit(1);
    }

    let res = build_post_request("fileOperations.redirect", &config)
        .body(body.to_string()).send().await.unwrap().bytes().await.unwrap();
    

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
            let url = res_text.split("<Resource>").nth(1).unwrap().split("</Resource>").nth(0).unwrap();
            let link = config.server.to_string() + &url;
            println!("{}", link);
            //download file from link and write to test.zip
            let mut file = std::fs::File::create("outline-backup.zip").unwrap();
            let client = Client::new();
            let response_text = client.get(link).send().await.unwrap().bytes().await.unwrap();
            let mut cursor = Cursor::new(response_text);
        
            std::io::copy(&mut cursor, &mut file).unwrap();
           
        }
     else {
        println!("Another error occured");
     }
    }
    else {
        //Download regularly
        let mut file = std::fs::File::create("outline-backup.zip").unwrap();
        let mut cursor = Cursor::new(res);
        std::io::copy(&mut cursor, &mut file).unwrap();
    }

    move_backup(&config.location);

    //delete the backup from outline
    response = build_post_request("fileOperations.delete", &config).body(body.to_string()).send();
    response_text = response.await.unwrap().text().await.unwrap();
    let deleteResponseBodyJson: Value = serde_json::from_str(&response_text).unwrap();
    println!("{}", deleteResponseBodyJson);

}

fn build_post_request(apicall: &str, config: &Config) -> reqwest::RequestBuilder {
    let mut post = reqwest::Client::new()
        .post(config.server.to_owned() + "/api/" + apicall);
    post = post.header("Content-Type", "application/json");
    post = post.header("authorization", "Bearer ".to_owned() + &config.apikey);
    post = post.header("Accept", "application/json");

    return post;
}

fn get_config() -> Config {
    let mut file = std::fs::File::open(CONFIGFILE).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let mut config: Config = toml::from_str(&contents).unwrap();

    if config.secure {
        config.server = format!("https://{}", config.server);
    }
    else {
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
            std::fs::rename("outline-backup.zip", "outline-backup-".to_owned() + &time_str + ".zip").unwrap(); 
        }
        else {
            let time = chrono::Local::now();
            let time_str = time.format("%Y-%m-%d-%H-%M-%S").to_string();
            
            //absolute meaning C:\ or starts with /
            if path.is_absolute() {
                std::fs::copy("outline-backup.zip", &path.join("outline-backup-".to_owned() + &time_str + ".zip")).unwrap();
                std::fs::remove_file("outline-backup.zip").unwrap();
            }
            else {
            std::fs::rename("outline-backup.zip", &path.join("outline-backup-".to_owned() + &time_str + ".zip")).unwrap(); 
            }
    }
}