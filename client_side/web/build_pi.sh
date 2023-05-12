# Can generally use trunk serve locally, but to run it on the pi itself...

set -exo pipefail

trunk build --release
# ...doesn't actually work because of permissions. oh well
# rsync nginx_server.conf pi@cnc:/etc/nginx/conf.d/cnc.conf
rsync -r dist/ pi@cnc:/home/pi/html
ssh -t pi@cnc sudo systemctl restart nginx