#/bin/bash

dotnet publish -r linux-x64 -p:PublishSingleFile=true --self-contained false
