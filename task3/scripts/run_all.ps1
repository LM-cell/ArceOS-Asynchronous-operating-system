$ErrorActionPreference = "Stop"

# Windows can run the basic comparison, but /proc/self/status metrics are only
# available on Linux. Use scripts/run_all.sh on Linux for report data.

$Models = @("os-thread", "green-thread", "async-future")
$Tasks = @(1000, 10000)
$CsvPath = "data/results.csv"
$SamplesPath = "data/samples.csv"
$FailureLog = "data/failures.log"
$LogDir = "data/logs"

Remove-Item -ErrorAction SilentlyContinue $CsvPath, $SamplesPath, $FailureLog
Remove-Item -ErrorAction SilentlyContinue -Recurse $LogDir
New-Item -ItemType Directory -Force $LogDir | Out-Null

Write-Host "Running models: $($Models -join ', ')"
Write-Host "Running task counts: $($Tasks -join ', ')"
Write-Host "Each model/task-count pair runs in a fresh process."

foreach ($model in $Models) {
    foreach ($tasks in $Tasks) {
        $RunLog = Join-Path $LogDir "$model-$tasks.log"
        Write-Host "==> model=$model tasks=$tasks"
        try {
            & cargo run --release -- `
                --models $model `
                --tasks $tasks `
                --sleep-ms 10 `
                --os-stack-kib 64 `
                --green-stack-kib 64 `
                --touch-stack-kib 8 `
                --kernel-stack-kib 16 `
                --sample-interval-ms 5 `
                --csv $CsvPath `
                --json "data/$model-$tasks.json" `
                --samples-csv $SamplesPath `
                --append-csv *> $RunLog
            if ($LASTEXITCODE -ne 0) {
                throw "cargo exited with code $LASTEXITCODE"
            }
        } catch {
            "failed model=$model tasks=$tasks log=$RunLog" | Tee-Object -Append $FailureLog
        }
    }
}

Write-Host "summary: $CsvPath"
Write-Host "samples: $SamplesPath"
Write-Host "failures: $FailureLog"
Write-Host "per-run logs: $LogDir"
