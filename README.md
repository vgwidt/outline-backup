Light backup tool for Outline wiki  (https://github.com/outline/outline).  Makes export_all request, gets the fileOperation ID, checks the status until it is complete, downloads it, then renames and moves it.

Using reqwest results in an error when using Minio (too many auth methods), but that error contains the JWT link so we extract that and use a simple get request to download it.  The program checks if the response length is of error length, if so, converts the response to text and analyzes it for the error and proceeds to extract the download URL.  Otherwise, it simply proceeds to copy the bytes provided from the redirect to outline-backup.zip.  Tested on hosted and self-hosted verson 0.65.2.

Be careful as rate limiting for full export requests has been added to Outline recently and may cause hung export requests.

1. Build using Cargo
2. Generate API key in Outline
3. Fill out settings.toml (see settings.toml.example)
4. Run

## To-Do
* Debugging
* Proper error handling
* Allow for time-based automation
* Allow for action-based automation