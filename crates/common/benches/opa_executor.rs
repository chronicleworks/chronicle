use common::{
    identity::AuthId,
    opa::{CliPolicyLoader, OpaExecutor, OpaExecutorError, WasmtimeOpaExecutor},
};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use tokio::runtime::Runtime;

fn load_wasm_from_file() -> Result<(), OpaExecutorError> {
    let wasm = "auth.tar.gz";
    let entrypoint = "auth.is_authenticated";
    CliPolicyLoader::from_embedded_policy(wasm, entrypoint)?;
    Ok(())
}

fn bench_policy_loader_load_policy(c: &mut Criterion) {
    c.bench_function("load_wasm_from_embedded", |b| {
        b.iter(|| load_wasm_from_file().unwrap());
    });
}

fn build_executor_from_loader(loader: &CliPolicyLoader) -> Result<(), OpaExecutorError> {
    let _executor = WasmtimeOpaExecutor::from_loader(loader)?;
    Ok(())
}

fn bench_build_opa_executor(c: &mut Criterion) {
    let input = {
        let wasm = "auth.tar.gz";
        let entrypoint = "auth.is_authenticated";
        CliPolicyLoader::from_embedded_policy(wasm, entrypoint).unwrap()
    };
    c.bench_with_input(
        BenchmarkId::new("build_executor_from_loader", "CliPolicyLoader"),
        &input,
        |b, input| {
            b.iter(|| build_executor_from_loader(input));
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
                let wasm = "auth.tar.gz";
                let entrypoint = "auth.is_authenticated";
                let loader = CliPolicyLoader::from_embedded_policy(wasm, entrypoint).unwrap();
                WasmtimeOpaExecutor::from_loader(&loader).unwrap()
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
