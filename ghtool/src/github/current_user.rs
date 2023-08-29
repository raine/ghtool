use cynic_github_schema as schema;

// Below is generated with https://generator.cynic-rs.dev using ./current_user.graphql,
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct CurrentUser {
    pub viewer: User,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct User {
    pub login: String,
}
