// The feed's old address, /news/rss.xml, still serves the same feed, so
// readers subscribed before News became Updates keep receiving it. A feed
// reader does not follow an HTML redirect.
export { GET } from '../updates/rss.xml';
