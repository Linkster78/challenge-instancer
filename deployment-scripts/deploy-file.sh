#!/bin/bash
set -eu

# Example deployment script that creates a file
# The arguments are passed as follows
#   $1 : command (can be start, stop or restart)
#   $2 : challenge_id
#   $3 : user_id
#
# To generate a unique identifier, the md5sum of the user_id should be used

uid_hash=$(echo -n "$3" | md5sum | head -c8)
filename="$2-$uid_hash"

exit 125

if [[ "$1" == "start" ]]; then
  echo "creating file $filename"
	touch "$filename"
	echo "\$ $(pwd)/$filename"
elif [[ "$1" == "stop" ]]; then
  echo "removing file $filename"
	rm "$filename"
elif [[ "$1" == "restart" ]]; then
  echo "recreating file $filename"
  rm "$filename"
  touch "$filename"
  echo "\$ $(pwd)/$filename"
fi
