use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Item {
    id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct NewItem {
    name: String,
}

type Db = Mutex<Vec<Item>>;

async fn get_items(db: web::Data<Db>) -> impl Responder {
    let db = db.lock().unwrap();
    HttpResponse::Ok().json(&*db)
}

async fn get_item(db: web::Data<Db>, id: web::Path<String>) -> impl Responder {
    let db = db.lock().unwrap();
    match db.iter().find(|item| &item.id == id.as_str()) {
        Some(item) => HttpResponse::Ok().json(item),
        None => HttpResponse::NotFound().body("Item not found"),
    }
}

async fn create_item(db: web::Data<Db>, new_item: web::Json<NewItem>) -> impl Responder {
    let mut db = db.lock().unwrap();
    let item = Item {
        id: Uuid::new_v4().to_string(),
        name: new_item.name.clone(),
    };
    db.push(item.clone());
    HttpResponse::Created().json(item)
}

async fn delete_item(db: web::Data<Db>, id: web::Path<String>) -> impl Responder {
    let mut db = db.lock().unwrap();
    let len_before = db.len();
    db.retain(|item| &item.id != id.as_str());
    if db.len() < len_before {
        HttpResponse::Ok().body("Item deleted")
    } else {
        HttpResponse::NotFound().body("Item not found")
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let db = web::Data::new(Mutex::new(Vec::<Item>::new()));

    println!("Starting server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(db.clone())
            .route("/items", web::get().to(get_items))
            .route("/items/{id}", web::get().to(get_item))
            .route("/items", web::post().to(create_item))
            .route("/items/{id}", web::delete().to(delete_item))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
