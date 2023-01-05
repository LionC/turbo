use anyhow::Result;
use turbo_tasks::primitives::StringsVc;
use turbo_tasks_fs::FileSystemPathVc;
use turbopack::module_options::{ModuleRule, ModuleRuleCondition, ModuleRuleEffect};
use turbopack_core::reference_type::{ReferenceType, UrlReferenceSubType};
use turbopack_ecmascript::{
    EcmascriptInputTransform, EcmascriptInputTransformsVc, NextJsPageExportFilter,
};

use super::context::SharedContextType;
use crate::{
    next_client::context::ClientContextType,
    next_server::context::{PageSsrType, ServerContextType},
};

/// Returns a list of module rules which apply Next.js-specific transforms.
pub async fn get_next_transforms_rules(context_ty: SharedContextType) -> Result<Vec<ModuleRule>> {
    let mut rules = vec![];

    match context_ty {
        SharedContextType::Server(ServerContextType::Pages {
            pages_dir,
            ssr_type,
        }) => {
            rules.push(get_next_dynamic_transform_rule(
                true,
                true,
                false,
                Some(pages_dir),
            ));

            match ssr_type {
                PageSsrType::Ssr => {}
                PageSsrType::SsrData => {
                    rules.push(
                        get_next_pages_transforms_rule(
                            pages_dir,
                            NextJsPageExportFilter::StripDefaultExport,
                        )
                        .await?,
                    );
                }
            }
        }
        SharedContextType::Server(ServerContextType::AppSSR { .. }) => {
            rules.push(get_next_dynamic_transform_rule(true, true, false, None));
        }
        SharedContextType::Server(ServerContextType::AppRSC { .. }) => {
            rules.push(get_next_dynamic_transform_rule(true, true, true, None));
        }
        SharedContextType::Client(client_context_type) => {
            rules.push(get_next_font_transform_rule());

            match client_context_type {
                ClientContextType::Pages { pages_dir } => {
                    rules.push(
                        get_next_pages_transforms_rule(
                            pages_dir,
                            NextJsPageExportFilter::StripDataExports,
                        )
                        .await?,
                    );
                    rules.push(get_next_dynamic_transform_rule(
                        true,
                        false,
                        false,
                        Some(pages_dir),
                    ));
                }
                ClientContextType::App { .. }
                | ClientContextType::Fallback
                | ClientContextType::Other => {
                    rules.push(get_next_dynamic_transform_rule(true, false, false, None));
                }
            }
        }
    }

    Ok(rules)
}

async fn get_next_pages_transforms_rule(
    pages_dir: FileSystemPathVc,
    export_filter: NextJsPageExportFilter,
) -> Result<ModuleRule> {
    // Apply the Next SSG transform to all pages.
    let strip_transform = EcmascriptInputTransform::NextJsStripPageExports(export_filter);
    Ok(ModuleRule::new(
        ModuleRuleCondition::all(vec![
            ModuleRuleCondition::all(vec![
                ModuleRuleCondition::ResourcePathInExactDirectory(pages_dir.await?),
                ModuleRuleCondition::not(ModuleRuleCondition::ResourcePathInExactDirectory(
                    pages_dir.join("api").await?,
                )),
                ModuleRuleCondition::not(ModuleRuleCondition::any(vec![
                    // TODO(alexkirsz): Possibly ignore _app as well?
                    ModuleRuleCondition::ResourcePathEquals(pages_dir.join("_document.js").await?),
                    ModuleRuleCondition::ResourcePathEquals(pages_dir.join("_document.jsx").await?),
                    ModuleRuleCondition::ResourcePathEquals(pages_dir.join("_document.ts").await?),
                    ModuleRuleCondition::ResourcePathEquals(pages_dir.join("_document.tsx").await?),
                ])),
            ]),
            ModuleRuleCondition::any(vec![
                ModuleRuleCondition::ResourcePathEndsWith(".js".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".jsx".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".ts".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".tsx".to_string()),
            ]),
        ]),
        vec![ModuleRuleEffect::AddEcmascriptTransforms(
            EcmascriptInputTransformsVc::cell(vec![strip_transform]),
        )],
    ))
}

fn get_next_dynamic_transform_rule(
    is_development: bool,
    is_server: bool,
    is_server_components: bool,
    pages_dir: Option<FileSystemPathVc>,
) -> ModuleRule {
    let dynamic_transform = EcmascriptInputTransform::NextJsDynamic {
        is_development,
        is_server,
        is_server_components,
        pages_dir,
    };
    ModuleRule::new(
        ModuleRuleCondition::all(vec![
            ModuleRuleCondition::not(ModuleRuleCondition::ReferenceType(ReferenceType::Url(
                UrlReferenceSubType::Undefined,
            ))),
            ModuleRuleCondition::any(vec![
                ModuleRuleCondition::ResourcePathEndsWith(".js".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".jsx".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".ts".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".tsx".to_string()),
            ]),
        ]),
        vec![ModuleRuleEffect::AddEcmascriptTransforms(
            EcmascriptInputTransformsVc::cell(vec![dynamic_transform]),
        )],
    )
}

fn get_next_font_transform_rule() -> ModuleRule {
    #[allow(unused_mut)] // This is mutated when next-font-local is enabled
    let mut font_loaders = vec!["@next/font/google".to_owned()];
    #[cfg(feature = "next-font-local")]
    font_loaders.push("@next/font/local".to_owned());

    ModuleRule::new(
        // TODO: Only match in pages (not pages/api), app/, etc.
        ModuleRuleCondition::all(vec![
            ModuleRuleCondition::not(ModuleRuleCondition::ReferenceType(ReferenceType::Url(
                UrlReferenceSubType::Undefined,
            ))),
            ModuleRuleCondition::any(vec![
                ModuleRuleCondition::ResourcePathEndsWith(".js".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".jsx".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".ts".to_string()),
                ModuleRuleCondition::ResourcePathEndsWith(".tsx".to_string()),
            ]),
        ]),
        vec![ModuleRuleEffect::AddEcmascriptTransforms(
            EcmascriptInputTransformsVc::cell(vec![EcmascriptInputTransform::NextJsFont(
                StringsVc::cell(font_loaders),
            )]),
        )],
    )
}
