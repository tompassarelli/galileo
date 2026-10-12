use galileo_core::*;
use galileo_core::supports::{failure_sets, Strength};
fn make(facts: u8, terms: &[(u8,u8)]) -> (Program,RelationId,Vec<RelationId>) {
    let mut b=Program::builder();
    let ids:Vec<_>=(0..4).map(|i|b.relation(Relation::new(format!("a{i}"),RelationKind::Static,[]))).collect();
    let s=b.relation(Relation::new("s",RelationKind::Derived,[]));
    for i in 0..4 {if facts&(1<<i)!=0 {b.fact(Fact::new(ids[i],[]));}}
    for (i,(pos,neg)) in terms.iter().enumerate() {
        let mut body=Vec::new();
        for j in 0..4 {
            if pos&(1<<j)!=0 {body.push(Atom::new(ids[j],[]).positive());}
            if neg&(1<<j)!=0 {body.push(Atom::new(ids[j],[]).negative());}
        }
        b.rule(Rule::new(format!("r{i}"),Atom::new(s,[])).when(body));
    }
    (b.build(),s,ids)
}
fn dnf(facts:u8,terms:&[(u8,u8)]) -> bool {
    terms.iter().any(|(pos,neg)|facts&pos==*pos && facts&neg==0)
}
fn main() {
    let mut seed=0x314_u64;
    let mut checks=0;
    for case in 0..16 {
        let mut terms=Vec::new();
        for _ in 0..4 {
            seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let pos=((seed>>32)&15) as u8;
            let neg=((seed>>40)&15) as u8 & !pos;
            terms.push((pos,neg));
        }
        for facts in 0..16_u8 {
            let (p,s,ids)=make(facts,&terms);
            let reference=evaluate(&p,&[],Budget::default()).unwrap();
            assert_eq!(reference.contains(s,&[]),dnf(facts,&terms),"DNF/reference case={case} facts={facts}");
            let expected=if !dnf(facts,&terms) {Some(0)} else {
                (0..16_u8).filter(|removed|removed&!facts==0 && !dnf(facts&!removed,&terms)).map(|r|r.count_ones() as usize).min()
            };
            let actual=failure_sets(&p,&[],s,&[],&ids,Budget::default()).unwrap();
            assert_eq!(actual.size(),expected,"case={case} facts={facts} terms={terms:?} actual={actual:?}");
            assert!(!matches!(actual,Strength::Unknown(_)),"unexpected Unknown case={case}");
            if let Strength::Smallest { sets, complete }=actual {
                assert!(complete,"incomplete full-budget search case={case}");
                for set in sets {
                    let removed=set.iter().fold(0_u8,|v,f|v|(1<<ids.iter().position(|r|*r==f.relation).unwrap()));
                    assert!(!dnf(facts&!removed,&terms));
                    assert!(set.len() >= expected.unwrap());
                }
            }
            checks+=1;
        }
    }
    println!("PASS: {checks} SPEC-grounded DNF/reference/strength cases, seed=0x314");
    let (p,s,ids)=make(7,&[(1,8)]);
    let base=evaluate(&p,&[],Budget::default()).unwrap().work;
    let mut removed=p.clone();
    removed.facts.retain(|f|f.relation!=ids[0]);
    let without=evaluate(&removed,&[],Budget::default()).unwrap().work;
    println!("Budget diagnostic base={base}, literal-removal={without}, strength-default={:?}",failure_sets(&p,&[],s,&[],&ids,Budget::default()).unwrap());
    let mut first_known=None;
    for budget in 0..256 {
        let actual=failure_sets(&p,&[],s,&[],&ids,Budget::work(budget)).unwrap();
        if !matches!(actual,Strength::Unknown(_)) {first_known=Some((budget,actual));break;}
    }
    println!("First conclusive budget={first_known:?}, budget14={:?}",failure_sets(&p,&[],s,&[],&ids,Budget::work(14)).unwrap());
    let (p,s,ids)=make(15,&[(1,0),(0,1)]);
    let mut first_known=None;
    for budget in 0..256 {
        let actual=failure_sets(&p,&[],s,&[],&ids,Budget::work(budget)).unwrap();
        if !matches!(actual,Strength::Unknown(_)) {first_known=Some((budget,actual));break;}
    }
    println!("Unbreakable budget sweep first-conclusive={first_known:?}");
}

