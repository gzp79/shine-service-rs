use axum::{
    body::HttpBody,
    handler::Handler,
    routing::{delete, get, post, put},
    Router,
};
use utoipa::openapi::{path::OperationBuilder, OpenApi, PathItemType};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ApiMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl From<ApiMethod> for PathItemType {
    fn from(value: ApiMethod) -> Self {
        match value {
            ApiMethod::Get => PathItemType::Get,
            ApiMethod::Post => PathItemType::Post,
            ApiMethod::Put => PathItemType::Put,
            ApiMethod::Delete => PathItemType::Delete,
        }
    }
}

pub trait ApiPath {
    fn path(&self) -> String;
}

impl ApiPath for String {
    fn path(&self) -> String {
        self.clone()
    }
}

pub struct ApiEndpoint<S, B> {
    method: ApiMethod,
    path: String,
    operation_id: Option<String>,
    description: Option<String>,
    tags: Vec<String>,

    router: Router<S, B>,
}

impl<S, B> ApiEndpoint<S, B>
where
    B: HttpBody + Send + 'static,
    S: Clone + Send + Sync + 'static,
{
    pub fn new<P, H, T>(method: ApiMethod, path: P, action: H) -> Self
    where
        P: ApiPath,
        H: Handler<T, S, B>,
        T: 'static,
    {
        let path = path.path();

        let router = Router::new().route(
            &path,
            match method {
                ApiMethod::Get => get(action),
                ApiMethod::Post => post(action),
                ApiMethod::Put => put(action),
                ApiMethod::Delete => delete(action),
            },
        );

        Self {
            method,
            path,
            operation_id: None,
            description: None,
            tags: Vec::new(),
            router,
        }
    }

    #[must_use]
    pub fn with_description<D: ToString>(mut self, description: D) -> Self {
        self.description = Some(description.to_string());
        self
    }

    #[must_use]
    pub fn with_tag<T: ToString>(mut self, tag: T) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    #[must_use]
    pub fn with_tags<I: IntoIterator<Item = String>>(mut self, tags: I) -> Self {
        self.tags = tags.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_operation_id<D: ToString>(mut self, operation_id: D) -> Self {
        self.operation_id = Some(operation_id.to_string());
        self
    }

    fn register(self, router: Router<S, B>, doc: Option<&mut OpenApi>) -> Router<S, B> {
        if let Some(doc) = doc {
            let operation = OperationBuilder::new()
                .operation_id(self.operation_id)
                .description(self.description)
                .tags(Some(self.tags))
                .build();

            let path_item = doc.paths.paths.entry(self.path).or_default();
            path_item.operations.insert(self.method.into(), operation);
        }

        router.merge(self.router)
    }
}

/// Helper trait to add ApiEndpoint to a Router
pub trait ApiRoute<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
{
    fn add_opt_api(self, endpoint: ApiEndpoint<S, B>, doc: Option<&mut OpenApi>) -> Self;

    fn add_api(self, endpoint: ApiEndpoint<S, B>, doc: &mut OpenApi) -> Self
    where
        Self: Sized,
    {
        self.add_opt_api(endpoint, Some(doc))
    }
}

impl<S, B> ApiRoute<S, B> for Router<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
{
    fn add_opt_api(self, endpoint: ApiEndpoint<S, B>, doc: Option<&mut OpenApi>) -> Self {
        endpoint.register(self, doc)
    }
}
