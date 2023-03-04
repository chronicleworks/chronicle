use common::{
    identity::{AuthId, IdentityContext, OpaData},
    opa::{CliPolicyLoader, OpaExecutor, OpaExecutorError, WasmtimeOpaExecutor},
};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use serde_json::Value;
use tokio::runtime::Runtime;

fn load_wasm_from_file() -> Result<(), OpaExecutorError> {
    let wasm = "allow_transactions";
    let entrypoint = "allow_transactions/allowed_users";
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
        let wasm = "allow_transactions";
        let entrypoint = "allow_transactions.allowed_users";
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

async fn evaluate(
    executor: &mut WasmtimeOpaExecutor,
    id: &AuthId,
    data: &OpaData,
) -> Result<(), OpaExecutorError> {
    executor.evaluate(id, data).await?;
    Ok(())
}

fn bench_evaluate_policy(c: &mut Criterion) {
    c.bench_function("evaluate", |b| {
        b.iter_batched_ref(
            || {
                let wasm = "allow_transactions";
                let entrypoint = "allow_transactions.allowed_users";
                let loader = CliPolicyLoader::from_embedded_policy(wasm, entrypoint).unwrap();
                let id = AuthId::chronicle();
                let data = OpaData::Operation(IdentityContext::new(
                    AuthId::chronicle(),
                    Value::default(),
                    Value::default(),
                ));
                (WasmtimeOpaExecutor::from_loader(&loader).unwrap(), id, data)
            },
            |(executor, id, data)| {
                let rt = Runtime::new().unwrap();
                let handle = rt.handle();
                handle.block_on(async move { evaluate(executor, id, data).await.unwrap() })
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
