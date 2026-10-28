$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$toolPython = Join-Path $projectRoot 'scratch\tooling\Scripts\python.exe'
if (-not (Test-Path -LiteralPath $toolPython)) {
    python -m venv scratch/tooling
}
& $toolPython -m pip install -r scripts/requirements-build.txt
if ($LASTEXITCODE -ne 0) { throw 'Installazione strumenti di build fallita' }
& $toolPython scripts/prepare_ocr.py
if ($LASTEXITCODE -ne 0) { throw 'Preparazione runtime OCR fallita' }
New-Item -ItemType Directory -Force runtime/models | Out-Null
$modelCommit = '87416418657359cb625c412a48b6e1d6d41c29bd'
$modelHashes = @{ eng='7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2'; ita='b8f89e1e785118dac4d51ae042c029a64edb5c3ee42ef73027a6d412748d8827' }
foreach ($modelLang in @('eng','ita')) {
    $modelPath = "runtime/models/$modelLang.traineddata"
    Invoke-WebRequest "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/$modelCommit/$modelLang.traineddata" -OutFile $modelPath
    if ((Get-FileHash $modelPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $modelHashes[$modelLang]) { throw "Hash modello $modelLang errato" }
}
Invoke-WebRequest "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/$modelCommit/LICENSE" -OutFile runtime/models/LICENSE
New-Item -ItemType Directory -Force scratch | Out-Null
Invoke-WebRequest 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8125/pdfium-win-x64.tgz' -OutFile scratch/pdfium.tgz
if ((Get-FileHash scratch/pdfium.tgz).Hash.ToLowerInvariant() -ne '9a964261760cff52570aabf038ab346d3a5034dd8b30d0d5fe8861235d356b8b') { throw 'Hash PDFium errato' }
tar -xf scratch/pdfium.tgz -C runtime
if ($LASTEXITCODE -ne 0) { throw 'Estrazione PDFium fallita' }
Write-Host 'Runtime preparati nella cartella di progetto; nessuna installazione di sistema.'
