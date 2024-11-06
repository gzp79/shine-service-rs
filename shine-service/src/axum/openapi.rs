use axum::{
    handler::Handler,
    http::StatusCode,
    routing::{delete, get, post, put, MethodRouter},
    Router,
};
use regex::Regex;
use utoipa::{
    openapi::{
        path::{OperationBuilder, Parameter, ParameterIn, PathItemBuilder},
        request_body::RequestBodyBuilder,
        ComponentsBuilder, Content, ContentBuilder, HttpMethod, OpenApi, OpenApiBuilder, PathsBuilder, Ref, Response,
        ResponseBuilder,
    },
    IntoParams, PartialSchema, ToResponse, ToSchema,
};

pub fn add_default_components(doc: &mut OpenApi) {
    #[derive(ToSchema)]
    //#[schema(value_type = String)]
    struct Url;

    #[derive(ToResponse)]
    #[allow(dead_code)]
    struct Problem {
        r#type: String,
        detail: Option<serde_json::Value>,
        instance: Option<Url>,
    }

    let components = ComponentsBuilder::new()
        .schema_from::<Url>()
        .response_from::<Problem>()
        .build();
    let new_doc = OpenApiBuilder::new().components(Some(components)).build();
    doc.merge(new_doc);
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ApiMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl From<ApiMethod> for HttpMethod {
    fn from(value: ApiMethod) -> Self {
        match value {
            ApiMethod::Get => HttpMethod::Get,
            ApiMethod::Post => HttpMethod::Post,
            ApiMethod::Put => HttpMethod::Put,
            ApiMethod::Delete => HttpMethod::Delete,
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

fn to_swagger(path: &str) -> String {
    let re = Regex::new(r":(\w+)").unwrap();
    re.replace_all(path, "{${1}}").to_string()
}

pub struct ApiEndpoint<S = ()> {
    method: ApiMethod,
    path: String,
    pub operation: OperationBuilder,
    pub components: ComponentsBuilder,
    router: MethodRouter<S>,
}

impl<S> ApiEndpoint<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new<P, H, T>(method: ApiMethod, path: P, action: H) -> Self
    where
        P: ApiPath,
        H: Handler<T, S>,
        T: 'static,
    {
        let path = path.path();

        let router = match method {
            ApiMethod::Get => get(action),
            ApiMethod::Post => post(action),
            ApiMethod::Put => put(action),
            ApiMethod::Delete => delete(action),
        };

        Self {
            method,
            path,
            operation: OperationBuilder::new(),
            components: ComponentsBuilder::new(),
            router,
        }
    }

    #[must_use]
    pub fn with_description<D: ToString>(mut self, description: D) -> Self {
        self.operation = self.operation.description(Some(description.to_string()));
        self
    }

    #[must_use]
    pub fn with_tag<T: ToString>(mut self, tag: T) -> Self {
        self.operation = self.operation.tag(tag.to_string());
        self
    }

    #[must_use]
    pub fn with_tags<I: IntoIterator<Item = String>>(mut self, tags: I) -> Self {
        for tag in tags {
            self.operation = self.operation.tag(tag.to_string());
        }
        self
    }

    #[must_use]
    pub fn with_operation_id<D: ToString>(mut self, operation_id: D) -> Self {
        self.operation = self.operation.operation_id(Some(operation_id.to_string()));
        self
    }

    #[must_use]
    pub fn with_parameter<P: Into<Parameter>>(mut self, parameter: P) -> Self {
        self.operation = self.operation.parameter(parameter);
        self
    }

    #[must_use]
    pub fn with_query_parameter<T: IntoParams>(mut self) -> Self {
        let params = <T as IntoParams>::into_params(|| Some(ParameterIn::Query));
        self.operation = self.operation.parameters(Some(params));
        self
    }

    #[must_use]
    pub fn with_path_parameter<T: IntoParams>(mut self) -> Self {
        let params = <T as IntoParams>::into_params(|| Some(ParameterIn::Path));
        self.operation = self.operation.parameters(Some(params));
        self
    }

    #[must_use]
    pub fn with_json_request<T>(mut self) -> Self
    where
        T: ToSchema,
    {
        let name = <T as ToSchema>::name();
        let schema = <T as PartialSchema>::schema();
        self.components = self.components.schema(name.clone(), schema);
        let content = Content::new(Some(Ref::from_schema_name(name)));
        let request = RequestBodyBuilder::new().content("application/json", content).build();
        self.operation = self.operation.request_body(Some(request));
        self
    }

    #[must_use]
    pub fn with_status_response<D: ToString>(mut self, code: StatusCode, description: D) -> Self {
        let response: Response = ResponseBuilder::new().description(description.to_string()).build();
        self.operation = self.operation.response(code.as_str().to_string(), response);
        self
    }

    #[must_use]
    pub fn with_schema<T>(mut self) -> Self
    where
        T: ToSchema,
    {
        let name = <T as ToSchema>::name();
        let schema = <T as PartialSchema>::schema();
        self.components = self.components.schema(name, schema);
        self
    }

    #[must_use]
    pub fn with_json_response<T>(mut self, code: StatusCode) -> Self
    where
        T: ToSchema,
    {
        let name = <T as ToSchema>::name();
        let schema = <T as PartialSchema>::schema();
        self.components = self.components.schema(name.clone(), schema);
        let content = ContentBuilder::new().schema(Some(Ref::from_schema_name(name))).build();
        let response = ResponseBuilder::new().content("application/json", content).build();
        self.operation = self.operation.response(code.as_str().to_string(), response);
        self
    }

    #[must_use]
    pub fn with_page_response<D: ToString>(mut self, description: D) -> Self {
        let content = ContentBuilder::new().schema(Some(String::schema())).build();
        let response = ResponseBuilder::new()
            .content("text/plan", content)
            .description(description.to_string())
            .build();
        self.operation = self.operation.response(StatusCode::OK.as_str().to_string(), response);
        self
    }

    #[must_use]
    pub fn with_problem_response(mut self, codes: &[StatusCode]) -> Self {
        for code in codes {
            self.operation = self
                .operation
                .response(code.as_str().to_string(), Ref::from_response_name("Problem"));
        }
        self
    }

    fn register(self, router: Router<S>, doc: Option<&mut OpenApi>) -> Router<S> {
        if let Some(doc) = doc {
            let components = self.components.build();
            let operation = self.operation.build();
            let method = self.method.into();

            let components_doc = OpenApiBuilder::new().components(Some(components)).build();
            doc.merge(components_doc);

            let paths = PathsBuilder::new()
                .path(
                    to_swagger(&self.path),
                    PathItemBuilder::new().operation(method, operation).build(),
                )
                .build();
            doc.paths.merge(paths);
        }

        router.route(&self.path, self.router)
    }
}

/// Helper trait to add ApiEndpoint to a Router
pub trait ApiRoute<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn add_opt_api(self, endpoint: ApiEndpoint<S>, doc: Option<&mut OpenApi>) -> Self;

    fn add_api(self, endpoint: ApiEndpoint<S>, doc: &mut OpenApi) -> Self
    where
        Self: Sized,
    {
        self.add_opt_api(endpoint, Some(doc))
    }
}

impl<S> ApiRoute<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn add_opt_api(self, endpoint: ApiEndpoint<S>, doc: Option<&mut OpenApi>) -> Self {
        endpoint.register(self, doc)
    }
}
