import fs from "node:fs";
import {pathToFileURL} from "node:url";
const [entry,input,output]=process.argv.slice(2);
const {evaluateRequest}=await import(pathToFileURL(entry).href);
const fixture=JSON.parse(fs.readFileSync(input,"utf8"));
const failures=[];
for(const row of fixture.cases){
 try {
  const actual=evaluateRequest(JSON.stringify(row.request));
  if(actual!==row.expected_wire)failures.push({id:row.id,actual,expected:row.expected_wire});
 } catch(error) { failures.push({id:row.id,exception:{name:error.name,message:error.message,code:error.code},expected:row.expected_wire}); }

}
fs.writeFileSync(output,JSON.stringify({executed:fixture.cases.length,passed:fixture.cases.length-failures.length,failures},null,2)+"\n");
console.log(JSON.stringify({executed:fixture.cases.length,passed:fixture.cases.length-failures.length,failure_ids:failures.map(x=>x.id)}));
