use reqwest;

pub async fn get_ip() -> String {
    let response = reqwest::get("https://ip.villers.website").await;
    match response {
        Ok(response) => {
            let body = response.text().await;
            match body {
                Ok(body) => {
                    return body.trim().to_string()
                }
                Err(e) => {
                    println!("{}", e)
                }
            }
        }
        Err(e) => {
            println!("{}", e)
        }
    }
    "unknown ipv4 address".to_string()
}