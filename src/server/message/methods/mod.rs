pub mod auth;
pub mod check;
pub mod edit;
pub mod fetch;
pub mod send;
pub mod bind;
pub mod new;
pub mod join;

#[macro_use] // This will allow macros to be imported into the scope
pub mod auth_macros {
    /// Require authentification
    macro_rules! require_auth {
        ($handler:ident, $method:ident) => {
            {
                let session = $handler.session.read().await;
                if !session.is_authenticated() {
                    $handler.send_error($method, "You aren't authenticated!".into()).await;
                    return;
                }
            }
        };
    }

    pub(crate) use require_auth;
}