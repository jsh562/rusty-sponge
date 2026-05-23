# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_rusty_sponge_global_optspecs
	string join \n a/append strict no-strict spill-mb= h/help V/version
end

function __fish_rusty_sponge_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_rusty_sponge_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_rusty_sponge_using_subcommand
	set -l cmd (__fish_rusty_sponge_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -l spill-mb -d 'Override the in-memory spill threshold (Default mode only; ignored in Strict mode). Default: 128 MiB' -r
complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -s a -l append -d 'Append to the target instead of replacing it (reads the existing file first, then concatenates stdin)'
complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -l strict -d 'Enable strict moreutils-compat mode. Rejects every Default-mode extension and emits byte-equal usage/error text vs moreutils sponge'
complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -l no-strict -d 'Explicitly disable strict mode (overrides `RUSTY_SPONGE_STRICT` env var and `argv[0] = sponge` auto-detect). Highest precedence'
complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -s V -l version -d 'Print version'
complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -a "completions" -d 'Emit shell completion scripts (Default mode only)'
complete -c rusty-sponge -n "__fish_rusty_sponge_needs_command" -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rusty-sponge -n "__fish_rusty_sponge_using_subcommand completions" -s h -l help -d 'Print help'
complete -c rusty-sponge -n "__fish_rusty_sponge_using_subcommand help; and not __fish_seen_subcommand_from completions help" -f -a "completions" -d 'Emit shell completion scripts (Default mode only)'
complete -c rusty-sponge -n "__fish_rusty_sponge_using_subcommand help; and not __fish_seen_subcommand_from completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
