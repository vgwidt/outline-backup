Light backup tool for Outline wiki  (https://github.com/outline/outline).  Makes export_all request, gets the fileOperation ID, checks the status until it is complete, downloads it, then renames and moves it.

Tested on hosted and self-hosted (with Minio) version 0.65.2. 

Be careful as rate limiting for full export requests has been added to Outline recently and may cause hung export requests.

1. Build using Cargo
2. Generate API key in Outline
3. Run and generate settings.toml file on first run (stores in %APPDATA%\Outback or ~/.config/Outback)

## To-Do
* Switch to menu with arguments for automatic backups
* Debugging and logging
* Improved error handling
* Run-as-service features
 * Allow for time-based automation
 * Allow for action-based automation
