#!/bin/bash
# Launch Personal Agent fully detached so it survives the parent shell exit.
export DISPLAY="${DISPLAY:-:0}"
cd /home/acoliver/projects/personal-agent
exec target/debug/personal_agent_gpui > /tmp/pa_run2.log 2>&1
