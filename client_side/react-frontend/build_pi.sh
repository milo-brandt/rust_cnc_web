# Can generally use trunk serve locally, but to run it on the pi itself...

set -exo pipefail

API_HOST=cnc:3000 npm run build
# ...doesn't actually work because of permissions. oh well
# rsync nginx_server.conf pi@cnc:/etc/nginx/conf.d/cnc.conf
rsync -r dist/ pi@cnc:/home/pi/html
ssh -t pi@cnc sudo systemctl restart nginx