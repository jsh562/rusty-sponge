
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'rusty-sponge' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'rusty-sponge'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'rusty-sponge' {
            [CompletionResult]::new('--spill-mb', '--spill-mb', [CompletionResultType]::ParameterName, 'Override the in-memory spill threshold (Default mode only; ignored in Strict mode). Default: 128 MiB')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'Append to the target instead of replacing it (reads the existing file first, then concatenates stdin)')
            [CompletionResult]::new('--append', '--append', [CompletionResultType]::ParameterName, 'Append to the target instead of replacing it (reads the existing file first, then concatenates stdin)')
            [CompletionResult]::new('--strict', '--strict', [CompletionResultType]::ParameterName, 'Enable strict moreutils-compat mode. Rejects every Default-mode extension and emits byte-equal usage/error text vs moreutils sponge')
            [CompletionResult]::new('--no-strict', '--no-strict', [CompletionResultType]::ParameterName, 'Explicitly disable strict mode (overrides `RUSTY_SPONGE_STRICT` env var and `argv[0] = sponge` auto-detect). Highest precedence')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Emit shell completion scripts (Default mode only)')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'rusty-sponge;completions' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'rusty-sponge;help' {
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Emit shell completion scripts (Default mode only)')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'rusty-sponge;help;completions' {
            break
        }
        'rusty-sponge;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
