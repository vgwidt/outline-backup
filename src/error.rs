pub fn validate_response(status: &str, error: &str) {
    match status {
        "401" => {
            println!("Error: {}", error);
            println!("The API key is invalid");
            std::process::exit(1);
        }
        "403" => {
            println!("Error: {}", error);
            println!("The API key specified has insufficient privileges");
            std::process::exit(1);
        }
        "404" => {
            println!("Error: {}", error);
            println!("Resource not found");
            std::process::exit(1);
        }
        "429" => {
            println!("Error: {}", error);
            println!("The rate limit has been exceeded");
            std::process::exit(1);
        }
        "500" => {
            println!("Error: {}", error);
            println!("Internal server error");
            std::process::exit(1);
        }
        _ => {}
    }
}