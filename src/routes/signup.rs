use actix_web::{
    web,
    Responder,
    HttpResponse,
};
use::sqlx::{
    Executor,
};
use chrono::Utc;
use sqlx::{
    PgPool,
    Postgres,
    Transaction,
};
use uuid::Uuid;
use crate::domain::NewSubscriber;
use crate::domain::SubscriberEmail;
use crate::domain::SubscriberName;
use crate::domain::SubscriberPassword;

struct FormSignUpData {
    email: String,
    first_name: String,
    last_name: String,
    password: String,
}

impl TryFrom<FormSignUpData> for NewSubscriber {
    type Error = String;
    fn try_from(value: FormSignUpData) -> Result<Self, Self::Error> {
        let email = SubscriberEmail::parse(value.email)?;
        let first_name = SubscriberName::parse(value.first_name)?;
        let last_name = SubscriberName::parse(value.last_name)?;
        let password = SubscriberPassword::parse(value.password)?;

        Ok(
            Self{ 
                email, 
                first_name, 
                last_name, 
                password 
        })
    }
}

pub async fn signup(
    form: web::Form<FormSignUpData>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, String> {
    // Get the form data and convert to a new subscriber
    let new_subscriber = form.0.try_into()?;
    // Insert the subscriber into the database
    // Use transaction to ensure that the subscriber is only inserted if all operations succeed
    let mut transaction = pool.begin().await;
    let subscriber_id = insert_subscriber(&new_subscriber, &mut transaction).await;
    // commit the the transaction to save subcriber in the 
    transaction.commit().await?;
    // Return a response to client
    Ok(HttpResponse::Ok().finish())
}


pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber, 
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO subscribers (id, email, first_name, last_name, password_hash, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        subcriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.first_name.as_ref(),
        new_subscriber.last_name.as_ref(),
        new_subscriber.password.hashed_password,
        Utc::now(),
    )?;
    transaction.execute(query).await?;
    Ok(subscriber_id)

}

