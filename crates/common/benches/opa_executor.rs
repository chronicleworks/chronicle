use common::{
    identity::AuthId,
    opa_executor::{
        CliPolicyLoader, OpaExecutor, OpaExecutorError, PolicyLoader, WasmtimeOpaExecutor,
    },
};
use criterion::{
    async_executor::FuturesExecutor, criterion_group, criterion_main, BatchSize, BenchmarkId,
    Criterion,
};
use tokio::runtime::Runtime;

async fn load_wasm_from_file(loader: &mut CliPolicyLoader) -> Result<(), OpaExecutorError> {
    loader.load_policy().await?;
    Ok(())
}

fn bench_policy_loader_load_policy(c: &mut Criterion) {
    c.bench_function("load_wasm_from_file", |b| {
        b.iter_batched_ref(
            || {
                let mut loader = CliPolicyLoader::new();
                loader.set_address("src/dev_policies/auth.wasm");
                loader.set_entrypoint("auth.is_authenticated");
                loader
            },
            |input| {
                let rt = Runtime::new().unwrap();
                let handle = rt.handle();
                handle.block_on(async move { load_wasm_from_file(input).await.unwrap() })
            },
            BatchSize::SmallInput,
        );
    });
}

async fn build_executor_from_loader(loader: &CliPolicyLoader) -> Result<(), OpaExecutorError> {
    let _executor = WasmtimeOpaExecutor::from_loader(loader)?;
    Ok(())
}

fn bench_build_opa_executor(c: &mut Criterion) {
    let input = {
        let mut loader = CliPolicyLoader::new();
        loader.set_address("src/dev_policies/auth.wasm");
        loader.set_entrypoint("auth.is_authenticated");
        let rt = Runtime::new().unwrap();
        let handle = rt.handle();
        handle.block_on(async move {
            loader.load_policy().await.unwrap();
            loader
        })
    };
    c.bench_with_input(
        BenchmarkId::new("build_executor_from_loader", "CliPolicyLoader"),
        &input,
        |b, input| {
            b.to_async(FuturesExecutor)
                .iter(|| build_executor_from_loader(input));
        },
    );
}

async fn evaluate(executor: &mut WasmtimeOpaExecutor) -> Result<(), OpaExecutorError> {
    let id = AuthId::chronicle();
    let context = serde_json::json!({});
    executor.evaluate(&id, &context).await?;
    Ok(())
}

fn bench_evaluate_policy(c: &mut Criterion) {
    c.bench_function("evaluate", |b| {
        b.iter_batched_ref(
            || {
                let mut loader = CliPolicyLoader::new();
                loader.set_address("src/dev_policies/auth.wasm");
                loader.set_entrypoint("auth.is_authenticated");
                let rt = Runtime::new().unwrap();
                let handle = rt.handle();
                handle.block_on(async move {
                    loader.load_policy().await.unwrap();
                    WasmtimeOpaExecutor::from_loader(&loader).unwrap()
                })
            },
            |input| {
                let rt = Runtime::new().unwrap();
                let handle = rt.handle();
                handle.block_on(async move { evaluate(input).await.unwrap() })
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_policy_loader_load_policy,
    bench_build_opa_executor,
    bench_evaluate_policy
);
criterion_main!(benches);
