# PSScriptAnalyzer settings for scripts/smoke-windows.ps1 and scripts/smoke/*.ps1 (the CI job
# smoke-windows lints with them).
@{
    Severity     = @('Error', 'Warning')
    ExcludeRules = @(
        # The scripts report to the CI log; Write-Host is their output.
        'PSAvoidUsingWriteHost',
        # Their state-changing helpers act on a throwaway test setup, never on the user's data.
        'PSUseShouldProcessForStateChangingFunctions'
    )
}
