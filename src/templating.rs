use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

pub struct HtmlTemplate<T>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => {
                tracing::error!("{}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "An error has occurred. Contact the admins.").into_response()
            }
        }
    }
}