<#
Firma Authenticode di TracciaDoc.exe (e degli altri .exe/.dll indicati) con signtool.

Il certificato non sta mai nel repository. Si sceglie con variabili d'ambiente:
  TRACCIADOC_SIGN_THUMBPRINT     impronta SHA1 di un certificato di firma codice nell'archivio
                                 CurrentUser\My o LocalMachine\My (anche su token/smart card);
oppure
  TRACCIADOC_SIGN_PFX            percorso di un file .pfx fuori dal repository;
  TRACCIADOC_SIGN_PFX_PASSWORD   password del .pfx (solo in variabile, mai su file o in argomenti
                                 registrati nei log).
Facoltativo:
  TRACCIADOC_SIGN_TIMESTAMP      URL RFC 3161 del servizio di marca temporale della CA. Senza
                                 marca temporale la firma smette di essere valida alla scadenza
                                 del certificato.

Uso: pwsh -NoProfile -File scripts/sign.ps1 [-Path dist\TracciaDoc\TracciaDoc.exe] [-AllowUntrusted]
-AllowUntrusted serve solo per provare lo script con un certificato self-signed: la firma viene
apposta ma la catena non e attendibile e Windows continua a mostrare un editore sconosciuto.
#>
param(
    [string[]]$Path = @('dist\TracciaDoc\TracciaDoc.exe'),
    [switch]$AllowUntrusted
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq 'x64' } | Sort-Object { $_.Directory.Parent.Name } | Select-Object -Last 1
if (-not $signtool) { throw 'signtool.exe non trovato: installare Windows SDK (Signing Tools)' }

$thumbprint = $env:TRACCIADOC_SIGN_THUMBPRINT
$pfx = $env:TRACCIADOC_SIGN_PFX
if ($thumbprint -and $pfx) { throw 'Indicare TRACCIADOC_SIGN_THUMBPRINT oppure TRACCIADOC_SIGN_PFX, non entrambi' }
if (-not $thumbprint -and -not $pfx) { throw 'Nessun certificato: impostare TRACCIADOC_SIGN_THUMBPRINT o TRACCIADOC_SIGN_PFX' }

$certArgs = @()
if ($thumbprint) {
    $thumbprint = ($thumbprint -replace '[^0-9A-Fa-f]', '').ToUpperInvariant()
    $cert = Get-ChildItem Cert:\CurrentUser\My, Cert:\LocalMachine\My | Where-Object Thumbprint -eq $thumbprint | Select-Object -First 1
    if (-not $cert) { throw "Certificato $thumbprint non presente in CurrentUser\My o LocalMachine\My" }
    $certArgs = @('/sha1', $thumbprint)
    if ($cert.PSParentPath -like '*LocalMachine*') { $certArgs += '/sm' }
} else {
    if (-not (Test-Path -LiteralPath $pfx)) { throw 'File .pfx non trovato' }
    $full = (Resolve-Path -LiteralPath $pfx).Path
    if ($full.StartsWith($projectRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Il .pfx non deve stare dentro il repository' }
    $certArgs = @('/f', $full)
    if ($env:TRACCIADOC_SIGN_PFX_PASSWORD) { $certArgs += @('/p', $env:TRACCIADOC_SIGN_PFX_PASSWORD) }
}
$timestampArgs = @()
if ($env:TRACCIADOC_SIGN_TIMESTAMP) {
    $timestampArgs = @('/tr', $env:TRACCIADOC_SIGN_TIMESTAMP, '/td', 'sha256')
} else {
    Write-Warning 'Nessuna marca temporale (TRACCIADOC_SIGN_TIMESTAMP): la firma scade con il certificato.'
}

foreach ($file in $Path) {
    if (-not (Test-Path -LiteralPath $file)) { throw "File da firmare non trovato: $file" }
    & $signtool.FullName sign /fd sha256 @certArgs @timestampArgs /d 'TracciaDoc' $file | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Firma non riuscita: $file" }
    $signature = Get-AuthenticodeSignature -LiteralPath $file
    Write-Host "$file : $($signature.Status) - $($signature.SignerCertificate.Subject)"
    $accepted = $signature.Status -eq 'Valid' -or ($AllowUntrusted -and $signature.Status -in 'UnknownError', 'NotTrusted')
    if (-not $accepted) {
        throw "Firma presente ma non valida ($($signature.Status)): serve un certificato di firma codice di una CA attendibile"
    }
}
