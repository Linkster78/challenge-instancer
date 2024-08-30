#!/bin/bash
set -eu

# Example deployment script that creates a file
#
# The arguments are passed as follows
#   $1 : command (can be start, stop, restart or cleanup)
#   $2 : challenge_id
#   $3 : user_id
#
# Start/Stop/Restart - self-explanatory
# Cleanup - Stop variant that shouldn't fail, called to fix error scenarios
#
# Deployment details are passed to the instancer by prefixing a line of stdout with '$'
#
# To generate a unique identifier, the md5sum of the user_id should be used

uid_hash=$(echo -n "$3" | md5sum | head -c8)

create_file() {
  filename="$1-$2"
  echo "creating file $filename"
	touch "$filename"
	echo "\$ $(pwd)/$filename"
}

remove_file() {
  filename="$1-$2"
  echo "removing file $filename"
	rm "$filename"
}

if [[ "$1" == "start" ]]; then
  create_file "$2" "$uid_hash"
elif [[ "$1" == "stop" ]]; then
  remove_file "$2" "$uid_hash"
elif [[ "$1" == "restart" ]]; then
  remove_file "$2" "$uid_hash"
  create_file "$2" "$uid_hash"
elif [[ "$1" == "cleanup" ]]; then
  remove_file "$2" "$uid_hash" || true
fi
