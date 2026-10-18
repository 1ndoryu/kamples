mod read;
mod types;
mod write;

pub use types::{
    ArticleAuthorSummary, ArticleCategoryCount, ArticleDetail, ArticleEmbed, ArticleMeta,
    ArticleSummary, CreateArticleParams, UpdateArticleParams,
};

pub struct ArticleRepository;
