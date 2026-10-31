// Generate cross-language behavior fixtures from the web application's locked dependencies.
// This script is a migration test tool; no JavaScript ships in the native app.
import { createEmptyCard, fsrs } from 'ts-fsrs';
import { mkdirSync, writeFileSync } from 'node:fs';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { gap } from '../src/lib/text.ts';

const destination=new URL('../native/crates/atlas-domain/tests/fixtures/',import.meta.url);
mkdirSync(destination,{recursive:true});
const compact=(c,seen)=>({due:c.due.getTime(),s:c.stability,d:c.difficulty,el:c.elapsed_days,sd:c.scheduled_days,ls:c.learning_steps,reps:c.reps,lapses:c.lapses,st:c.state,...(c.last_review?{last:c.last_review.getTime()}:{}),seen});
const rows=[];
for(const retention of [0.7,0.85,0.9,0.95,0.97]){
 const scheduler=fsrs({request_retention:retention,enable_fuzz:true,enable_short_term:true,learning_steps:['1m','10m'],relearning_steps:['10m']});
 let now=Date.UTC(2026,9,30,15,59,20);
 let card=createEmptyCard(new Date(now));
 const seen=now;
 for(let index=0;index<90;index++){
  const previous=index?compact(card,seen):null;
  const choices=scheduler.repeat(card,new Date(now));
  rows.push({retention,now,previous,expected:[1,2,3,4].map(grade=>compact(choices[grade].card,seen))});
  const grade=index%13===7?1:index%7===0?2:index%11===9?4:3;
  card=choices[grade].card;
  now=card.due.getTime()+(index%3===0?86_400_000:index%3===1?0:1234);
 }
}
writeFileSync(new URL('fsrs-web-v5.4.2.json',destination),JSON.stringify(rows));
console.log(`Generated ${rows.length} scheduling cases, ${rows.length*4} expected card states.`);

const words=JSON.parse(readFileSync(new URL('../data/word-atlas.json',import.meta.url),'utf8'));
const index=JSON.parse(readFileSync(new URL('../public/atlas/index.json',import.meta.url),'utf8'));
const byId=new Map(words.map(word=>[word.id,word]));
const hash=createHash('sha256');let count=0;
for(const row of index.words){
 const word=byId.get(row[0]);
 word.ex.forEach((example,i)=>{
  const match=gap(example.en,word.w);
  hash.update(JSON.stringify([word.id,i,match?.before??null,match?.hidden??null,match?.after??null])+'\n');count++;
 });
}
writeFileSync(new URL('cloze-web.json',destination),JSON.stringify({examples:count,sha256:hash.digest('hex')})+'\n');
console.log(`Generated a cloze behavior digest for all ${count} canonical examples.`);
