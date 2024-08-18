#!/bin/bash
set -eu

# The arguments are passed as follows
#   $1 : command (can be start, stop or restart)
#   $2 : challenge_id
#   $3 : user_id

uid_hash=$(echo -n "$3" | md5sum | head -c8)

create_container() {
  echo "copying container $1-template"
  incus copy "$1-template" "$1-$2"
  echo "starting container $1-$2"
  incus start "$1-$2"

  while
    ctn_ip=$(incus ls -c4 -fcsv "$1-$2" | cut '-d ' -f1)
    [[ -z $ctn_ip ]]
  do true; done

  for port in $(incus config get "$1-template" user.exposed_ports | tr ',' '\n')
  do
      echo "\$ $ctn_ip:$port"
  done
}

remove_container() {
  echo "deleting container $1-$2"
  incus rm -f "$1-$2"
}

if [[ "$1" == "start" ]]; then
  create_container "$2" "$uid_hash"
elif [[ "$1" == "stop" ]]; then
  remove_container "$2" "$uid_hash"
elif [[ "$1" == "restart" ]]; then
  remove_container "$2" "$uid_hash"
  create_container "$2" "$uid_hash"
fi
