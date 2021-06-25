use actix_web::{
    get,
    web::{Data, HttpRequest, Json},
};
use houseflow_auth_types::{WhoamiResponse, WhoamiResponseBody, WhoamiResponseError};
use houseflow_config::server::Secrets;
use houseflow_db::Database;
use houseflow_token::Token;
use houseflow_types::UserAgent;

#[get("/whoami")]
pub async fn on_whoami(
    app_data: Data<Secrets>,
    db: Data<dyn Database>,
    req: HttpRequest,
) -> Result<Json<WhoamiResponse>, WhoamiResponseError> {
    let token = Token::from_request(&req)?;
    token.verify(&app_data.access_key, Some(&UserAgent::Internal))?;
    let user = db
        .get_user(token.user_id())
        .await
        .map_err(|err| WhoamiResponseError::InternalError(err.to_string()))?
        .ok_or(WhoamiResponseError::UserNotFound)?;

    let response = WhoamiResponseBody {
        username: user.username,
        email: user.email,
    };

    Ok(Json(WhoamiResponse::Ok(response)))
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::test_utils::*;
//     use actix_web::{http, test, App, ResponseError};
//     use types::User;

//     use rand::random;

//     #[actix_rt::test]
//     async fn valid() {
//         let database = get_database();
//         let secrets: Secrets = random();
//         let user = User {
//             id: random(),
//             username: String::from("John Smith"),
//             email: String::from("john_smith@example.com"),
//             password_hash: PASSWORD_HASH.into(),
//         };
//         let token = Token::new_access_token(&secrets.access_key, &user.id, &UserAgent::Internal);
//         database.add_user(&user).await.unwrap();
//         let mut app =
//             test::init_service(App::new().configure(|cfg| {
//                 crate::configure(cfg, get_token_store(), database, secrets.clone())
//             }))
//             .await;

//         let request = test::TestRequest::get()
//             .uri("/whoami")
//             .append_header((
//                 http::header::AUTHORIZATION,
//                 format!("Bearer {}", token.to_string()),
//             ))
//             .to_request();
//         let response = test::call_service(&mut app, request).await;
//         assert_eq!(
//             response.status(),
//             200,
//             "status is not succesfull, body: {:?}",
//             test::read_body(response).await
//         );
//         let response: WhoamiResponse = test::read_body_json(response).await;
//         let response = match response {
//             WhoamiResponse::Ok(response) => response,
//             WhoamiResponse::Err(err) => panic!("unexpected error: {}", err),
//         };
//         assert_eq!(user.email, response.email);
//         assert_eq!(user.username, response.username);
//     }

//     #[actix_rt::test]
//     async fn missing_header() {
//         let database = get_database();
//         let secrets: Secrets = random();
//         let user = User {
//             id: random(),
//             username: String::from("John Smith"),
//             email: String::from("john_smith@example.com"),
//             password_hash: PASSWORD_HASH.into(),
//         };
//         database.add_user(&user).await.unwrap();
//         let mut app =
//             test::init_service(App::new().configure(|cfg| {
//                 crate::configure(cfg, get_token_store(), database, secrets.clone())
//             }))
//             .await;

//         let request = test::TestRequest::get().uri("/whoami").to_request();
//         let response = test::call_service(&mut app, request).await;
//         const EXPECTED_ERROR: WhoamiResponseError =
//             WhoamiResponseError::DecodeHeaderError(token::DecodeHeaderError::MissingHeader);

//         assert_eq!(
//             response.status(),
//             EXPECTED_ERROR.status_code(),
//             "unexpected status: {}, body: {:?}",
//             response.status(),
//             test::read_body(response).await
//         );
//         let response: WhoamiResponseError = test::read_body_json(response).await;
//         assert_eq!(response, EXPECTED_ERROR);
//     }

//     #[actix_rt::test]
//     async fn invalid_token_signature() {
//         let database = get_database();
//         let secrets: Secrets = random();
//         let user = User {
//             id: random(),
//             username: String::from("John Smith"),
//             email: String::from("john_smith@example.com"),
//             password_hash: PASSWORD_HASH.into(),
//         };
//         let token = Token::new_access_token(&"dsahsdadsh", &user.id, &UserAgent::Internal);
//         database.add_user(&user).await.unwrap();
//         let mut app =
//             test::init_service(App::new().configure(|cfg| {
//                 crate::configure(cfg, get_token_store(), database, secrets.clone())
//             }))
//             .await;

//         let request = test::TestRequest::get()
//             .uri("/whoami")
//             .append_header((
//                 http::header::AUTHORIZATION,
//                 format!("Bearer {}", token.to_string()),
//             ))
//             .to_request();
//         let response = test::call_service(&mut app, request).await;
//         const EXPECTED_ERROR: WhoamiResponseError =
//             WhoamiResponseError::VerifyError(token::VerifyError::InvalidSignature);

//         assert_eq!(
//             response.status(),
//             EXPECTED_ERROR.status_code(),
//             "unexpected status: {}, body: {:?}",
//             response.status(),
//             test::read_body(response).await
//         );
//         let response: WhoamiResponseError = test::read_body_json(response).await;
//         assert_eq!(response, EXPECTED_ERROR);
//     }
// }