export function tagNameKey(name:string){return name.trim().normalize('NFC').toLowerCase().normalize('NFC');}
function resemblanceKey(name:string){return name.trim().normalize('NFKC').toLowerCase().replace(/\s+/gu,' ').normalize('NFC');}
export function similarTagNames(name:string,tags:{id:string;name:string}[],excludeId?:string){
 if(!name.trim())return [];
 const exact=tagNameKey(name),similar=resemblanceKey(name);
 return tags.filter(tag=>tag.id!==excludeId&&tagNameKey(tag.name)!==exact&&resemblanceKey(tag.name)===similar);
}
