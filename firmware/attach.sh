#!/usr/bin/env bash

chip=STM32F303VCTx
extra_args=()

if [[ $1 = -h || $1 == --help ]]; then
	cat <<-END
		Usage: attach.sh [OPTIONS]
		Attach to a running RTT session.

		OPTIONS:
		  -r, --reset: Reset before attaching
		  -h, --help: Show this message and exit
	END
	exit 0
fi

if [[ $1 = -r || $1 = --reset ]]; then
	extra_args+=(--cycle-power)
fi

exec probe-rs attach --chip "$chip" --log-format '{s}' --no-timestamps "${extra_args[@]}"
