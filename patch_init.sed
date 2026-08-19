/op_cognitive_mcp::typed_tools::register_typed_tools(/,/)\.await?;/!b
/)\.await?;/a\
\
        // Restore Phase 1 RagPipeline and ContextAwarenessEngine\
        let rag_pipeline = match op_cognitive_mcp::RagPipeline::from_env() {\
            Ok(p) => Some(std::sync::Arc::new(p)),\
            Err(e) => {\
                tracing::warn!(error = %e, "RAG pipeline unavailable; code-context tools will not be registered");\
                None\
            }\
        };\
\
        let rag_collection = op_cognitive_mcp::default_collection_from_env();\
        let context_engine = std::sync::Arc::new(op_cognitive_mcp::ContextAwarenessEngine::new(\
            op_cognitive_mcp::ContextAwarenessConfig {\
                rag_collection: rag_collection.clone(),\
                ..Default::default()\
            },\
            memory_store.clone(),\
            rag_pipeline.clone(),\
        ));\
        context_engine.clone().start_monitoring();\
\
        if let Some(rag) = &rag_pipeline {\
            let n = op_cognitive_mcp::register_code_tools(\
                &registry,\
                rag.clone(),\
                context_engine.clone(),\
                rag_collection,\
            )\
            .await?;\
            tracing::info!(registered = n, "Registered code-context tools in bridge");\
        }
