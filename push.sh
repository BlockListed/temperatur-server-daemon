#!/bin/sh
echo -e "\e[0;32mChecking SQLX Queries\e[0m"
cargo sqlx prepare

echo -e "\e[0;32mPushing to GitHub\e[0m"
git push