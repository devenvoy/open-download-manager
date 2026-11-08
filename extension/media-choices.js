(function(root){
  function assetKey(value) {
    try {
      const u=new URL(value);
      if (!/(^|\.)twimg\.com$/.test(u.hostname)) return '';
      return /\/(?:amplify_video|ext_tw_video|tweet_video)(?:_thumb)?\/(\d+)/.exec(u.pathname)?.[1] || '';
    } catch { return ''; }
  }
  function mediaChoices(items,context={}) {
    const wanted=assetKey(context.url)||assetKey(context.poster);
    if(items.some(i=>i.provider==='youtube'))return items.filter(i=>i.provider==='youtube');
    let list=items.filter(i=>!/\.(m4s|ts|aac)(?:[?#]|$)/i.test(i.url)&&!/^https:\/\/(?:www\.)?youtube\.com\/s\//.test(i.url));
    if(wanted) list=list.filter(i=>assetKey(i.url)===wanted);
    const richAssets=new Set(list.filter(i=>i.masterUrl).map(i=>assetKey(i.url)).filter(Boolean));
    const audioUrls=new Set(list.map(i=>i.audioUrl).filter(Boolean));
    list=list.filter(i=>!audioUrls.has(i.url) && (!richAssets.has(assetKey(i.url))||i.masterUrl));
    const seen=new Set(), choices=[], unsupportedChoices=[];
    for(const item of list.sort((a,b)=>(b.bitrate||0)-(a.bitrate||0))) {
      if(item.unsupported) {
        if(!seen.has(item.url)) {
          seen.add(item.url);
          unsupportedChoices.push({
            ...item,
            quality: item.quality || 'Unsupported',
            format: item.format || 'STREAM',
            audioState: 'Cannot be downloaded',
            unsupported: true,
            reason: item.reason || 'This stream format is not supported.'
          });
        }
        continue;
      }
      const hls=item.format==='HLS';
      const mime=String(item.mime||'').toLowerCase();
      const codecs=item.codecs||/codecs\s*=\s*"?([^";]+)/i.exec(item.mime||'')?.[1]||'';
      const extension=/\.([a-z0-9]+)(?:[?#]|$)/i.exec(item.url)?.[1]?.toLowerCase()||'';
      let inferredFormat = extension;
      if (/^(?:video|audio)\/webm/.test(mime)) inferredFormat = 'webm';
      else if (/^video\/(?:mp4|x-m4v|mp4v)/.test(mime)) inferredFormat = 'mp4';
      else if (/^audio\/(?:mpeg|mp3)/.test(mime)) inferredFormat = 'mp3';
      else if (/^video\/(?:quicktime|x-quicktime)/.test(mime)) inferredFormat = 'mov';
      else if (/^video\/(?:x-msvideo|avi|msvideo)/.test(mime)) inferredFormat = 'avi';
      else if (/^video\/(?:x-flv|flv|x-f4v|f4v)/.test(mime)) inferredFormat = 'flv';
      else if (/^video\/(?:x-ms-wmv|wmv|x-ms-asf|asf)/.test(mime)) inferredFormat = 'wmv';
      else if (/^video\/(?:x-matroska|mkv)/.test(mime)) inferredFormat = 'mkv';
      else if (/^video\/(?:mpeg|mpg)/.test(mime)) inferredFormat = 'mpg';
      else if (/^(?:video|audio)\/ogg/.test(mime)) inferredFormat = 'ogg';

      const format=item.format||inferredFormat.toUpperCase();
      const audioState=item.audioUrl?'Video + audio':/^audio\//.test(mime)?'Audio only':/mp4a|ac-3|ec-3|opus/i.test(codecs)?'Video + audio':codecs?'Video only (silent)':'Audio not verified';
      const outputFormat=item.outputFormat||(hls?'mp4':['mp4','webm','mp3','mov','avi','mkv'].includes(inferredFormat)?inferredFormat:undefined);
      const video={...item,format:hls?'MP4':format,outputFormat,audioState,asset:assetKey(item.url),unsupported:false};
      const variants=[video];
      if(item.audioUrl)variants.push({...item,url:item.audioUrl,audioUrl:undefined,format:'MP3',outputFormat:'mp3',quality:'',bitrate:0,audioState:'Audio only',asset:assetKey(item.url),unsupported:false});
      for(const choice of variants){const key=choice.url+'|'+(choice.outputFormat||'');if(!seen.has(key)){seen.add(key);choices.push(choice);}}
    }
    return choices.length ? choices : unsupportedChoices;
  }
  root.mediaChoices=mediaChoices;
  if(typeof module!=='undefined')module.exports={mediaChoices,assetKey};
})(globalThis);
