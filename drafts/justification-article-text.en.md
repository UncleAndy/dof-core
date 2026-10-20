# The Moral Theorem

## Preface

I am writing this not because of an argument about morality, but because decisions are now made by more than a person.

Models already choose who gets a loan, whom to invite for an interview, which application to reject, where to route a trip. More is coming: agents with access to money, to other people's data, to machinery and to weapons. The danger here is not the evil will of a machine. A machine does not wish evil; it has nothing to wish with. The danger is that we have already handed decisions over to it, while we cannot properly state what must not be done in those decisions. "Be honest", "do no harm", "respect rights" sound right and check nothing: you cannot compute an option from them, and you cannot say which rule exactly was broken. While a rule cannot be checked, it rests on the good will of whoever executes it, and the executor may have no good will at all. So a formal description from which norms are derived and checked stops being a good idea and becomes a condition of our survival. Either we manage to write down what must not be done in a form that can be computed and contested, or someone else writes it down for us.

All my life I watched people and could not understand one thing. Why does a person who knows firmly that lying is wrong lie when it pays, and not consider himself bad? Why is the same act grounds for a trial in one country, ordinary practice in another, and a duty a hundred years ago? If the measure of good depends on the place and the year, there is nothing to argue about: the one with more force wins, and whoever called his opinion morality the loudest. It always seemed to me a defect in how morality is written down, not a property of morality. A rule that changes depending on who pronounces it is a custom. It lasts exactly as long as it is repeated.

Much later I came across a paper I could not forget. Wissner-Gross and Freer, in Physical Review Letters in 2013, ran models that had no goal, no training on examples and no hints: the only thing those models did was preserve as many options for future action as they could. On their own, without instructions, they arrived at behaviour we would call reasonable and cautious: they avoided positions they could not get out of, and kept hold of the ability to act further. There is not a word about morality in it, and the conclusion is not about good. But the main thing is said: the drive to preserve one's degrees of freedom can be computed, and behaviour follows from it mathematically. You would not call that measure anything else. I have been thinking about it, about its references and about how far it can be carried, for more than ten years.

Here is what came out of those thoughts. The article you are reading has no new law of morality and no personal faith of mine that you have to take on my word. It has two statements and an attempt to derive everything else from them: what counts as well-being, what must not be done at someone else's expense, what to do about what we do not know. Everything after that is checked step by step, and that is the whole point. If the conclusion is right, it is right without my participation and without reference to me. If it is wrong, people will catch it as an error, not as disagreement with me. That is, the conclusions given in this article claim to be objective.

The DOF-CORE standard, the formula in it and four independent implementation programs are already a record of that derivation. The justification follows.

## Part I. The goal

### Happiness for all, for free, and may no one leave offended

Almost everyone knows this line, and almost everyone is wrong about who said it. In Roadside Picnic it is printed in capitals, because it is not spoken but shouted: "HAPPINESS FOR ALL, FOR FREE, AND MAY NO ONE LEAVE OFFENDED!". The words belong to Arthur Burbridge, a young idealist who died on the way to the Golden Sphere; in the finale Redrick Schuhart repeats them in despair, already as a prayer. The line is usually attributed to Schuhart, and that is understandable, but a mistake: the hero repeats someone else's words because he has none of his own. That, if you like, is the whole novel.

I need this line not as decoration. It holds four demands, and this article does something with each of them.

"For all" stays as it is. Not a majority, not "on average across the country", not "across the world as a whole": everyone who is counted at all. As soon as someone is put outside the brackets, it is a different goal, just not named aloud.

"May no one leave offended" holds, and is even strengthened. We "compensate" no one: you cannot replace someone's loss with someone else's well-being, because that replacement is the main lie of every promise that "things will get better for everyone".

"For free" I cross out. Promising for free is dishonest, and a path that cost nothing usually leads nowhere.

"Happiness" I replace. Happiness cannot be added up, compared or handed to another; a person is happy within a very narrow corridor of possibilities and thoroughly unhappy within a wide one. We will count not happiness but the paths a person has left.

And one last thing. The characters of the novel had a Sphere that granted wishes. We have no Sphere. We have mathematics, and it answers not the question "what to give" but the question "what must not be done". A weak consolation? Perhaps. But it is the only part of the line that can be fulfilled without a miracle.

### Why "the good of all" is not counted as an average

Let us take the ordinary "so that everyone is well" and try to count it. Three methods look obvious, and all three permit sacrifice by construction.

Sum. Take from one and give to two: it grew, so "it got better". Loss here is simply a negative term that anyone can outweigh.

Average. One drowned, three learned to swim. On average we are swimming. The average does not distinguish what a drowned man is: for it he is a smaller number.

Majority. Two out of three voted for taking. Formally it is all fair, in fact it was taken.

All three methods share one property: in them the loss of one is not a loss but a smaller number. A number can be outweighed, a loss cannot. So we need not another arithmetic of the same numbers, but a form of accounting in which bringing someone's future close to zero cannot be bought with any amount of someone else's good.

## Part II. The formula

### Nash's formula: a product instead of a sum

Such a form exists, and I did not invent it. In 1950 John Nash analysed the bargaining problem: how two sides can agree so that the result does not depend on the units in which we wrote down the payoff, so that the strong cannot push the weak around, and so that one side's position cannot be improved without worsening the other's. All three conditions at once are held only by the product of individual quantities. Addition loses them.

We are interested in exactly one property of the product: if even one factor is zero, the whole product is zero, however large the rest. In this arithmetic you cannot pay for someone's loss with your own gain. Not because conscience commands it, but because zero does not multiply in your favour.

Why not a sum and why not an average is clear from the previous chapter: they permit sacrifice. Why not maximin, which gives the worst off the maximum? Maximin pulls out the worst off but does not distinguish options in which the worst off is equally badly off while everyone else fares differently. The product does distinguish: it is sensitive both to the worst off and to everyone else.

And here the reader is obliged to ask: the product of what? So far it is an empty place. The factors are what we measure well-being with, and we have not yet said what that is.

### What exactly is changed in DOF-CORE and why that is not a substitution

The open DOF-CORE standard writes the formula like this:

```text
TotalDoF_index(S) = Σ_{e ∈ calc(S)} ln(DoF(e))
```

It reads like this: for each counted entity we take the logarithm of the number of its possibilities, and it all is added up. A sum of logarithms is the logarithm of a product, that is you are looking at the same Nash formula, only in a form convenient for counting: multiplying is awkward, adding is possible, and identically in any programming language.

But it is a modification, and its differences have to be named. There is no reason to hide them. There are four.

First: what goes under the logarithm is not the number of paths itself but either it or a very small positive constant, whichever is larger. This is a computational trick: the logarithm of zero does not exist, and without it the formula would fall over on the very first entity that has no paths left. The standard states separately that the constant is not a "price of death" and not permission to trade in destruction.

Second: the set of the counted is fixed before options are compared and does not change while they are compared. This is not pedantry. If an entity that an option destroys simply drops out of the sum, then destruction raises the result. One of the four independent ports once erred in exactly this way. The error was found not in the mathematics but in one line that took the set afresh.

Third: an option that drives a living entity to zero and closes its recovery is not admitted to comparison at all. Not fined, but simply not admitted: a fine can be outweighed, a prohibition cannot.

Fourth: under uncertainty we take the worst of the plausible futures, not the average across them. A gain under one reading does not pay for a catastrophe under another.

And one caveat that cannot be dropped. We are talking about an index: it orders options, it does not measure well-being in any units. We can say that one option is better than another, but we cannot say how large each option is. For the problem at hand that is enough.

## Part III. What well-being is

### Why "well-being" cannot be left as a word

Before substituting factors into the formula, we have to decide what this number is. Three habitual answers do not fit, each for its own reason.

Money does not fit, because with it you can have almost everything and lack the one thing you need: health, permission, time, a person nearby. And the other way round: a person may have little money and many paths.

Preferences do not fit, because any preferences can be cultivated in oneself, including slavish ones. A person is capable of sincerely valuing what destroys him; one can get used to anything, including a cage. After that "I am fine as I am" becomes not an argument but a lock on the door.

Self-assessment does not fit for the same reason: it measures habit, not the set of possibilities.

One thing is left: what a person can really do and really make use of. Not "what he feels" but "what he can do".

### Well-being is freedom, and freedom is the number of possibilities

Let us call things by their names. A person's well-being is his freedom, and freedom is the number of paths he has and which really lead somewhere. A tool you can work with. A road you can leave by. A contract you can rely on. A queue to the doctor you can actually join. A right you can use in practice, not on paper.

A path broken at the first step does not count. Otherwise freedom would be any formal permission, and there are plenty of those around.

Three protections against substitutions are needed here.

Freedom is not permissiveness. A path without consequences and without obligations is not a path but a dream; it does not survive the first collision with reality.

Freedom is not the absence of obligations. An obligation often widens paths: a contract gives possibilities that would not exist without it; a promise kept does too.

A right on paper is not a path. If the law allows it and in practice you cannot, there is no path.

Now we can return to the formula. The factor in the product is this very quantity. And "universal well-being" is what you get if you multiply it across everyone who is counted.

## Part IV. Time

### Growth matters more than level, and why the future cannot be summed

A level is eaten up in a day. The growth of the irreversible never comes back. So the question for any decision is not "how much we have" but "how much will be left afterwards".

Then begins the place where I must separate my own from the standard. Summing future levels with coefficients is meaningless, and the reason is not convenience: the destroyed does not enter the sum with any weight. Zero multiplied by anything stays zero. So the rule is formulated for each step, not "on average over a century". The standard has no such integration over time: it compares options here and now. And what I say about growth is my own consequence from irreversibility, not its norm.

For the same reason, time enters the standard not as a discount on the future but as a resource: how much is left before a catastrophe (or, for instance, before natural death). In plain words it is not "how profitable" but "how urgent".

## Part V. Consequences

### People are not a herd

The average has room for the one who lost everything too: a house does not come back because the neighbours gained. One person does not replace another, even if the one who takes his place will be happier: it is another life, not the same one.

This is not a new morality. It is the golden rule and Kant's claim: "a human being is an end, not a means". The difference is only that here it is not an appeal to conscience but a consequence of the chosen form of accounting, and it can be checked, not merely felt.

### Someone else's future cannot be bought, and consent does not help

The most unpleasant consequence, and there is no need to soften it. Ordinary morality permits a small evil for a great good; here it does not, because there is nothing with which to pay for a loss.

Consent does not save it either. A person can give up his future, but the loss remains a loss, and in such an exchange there is still nothing to count except it. Let me note that the standard does not mention consent even once: this is not a separate rule but a consequence of the absence of such an input in the accounting.

The only exception is the one who himself destroys someone else's future: life, health, the freedom to act. He is stopped and taken out of the common game. Not "deducted" in a report and not fined, but taken out. This is self-defence, familiar to everyone from ordinary law, and it does not make the ban on compensation conditional.

### The irreversible requires strong justification

A felled tree, a destroyed city, a right given away forever, an exterminated species. You can make a mistake; you cannot undo it. The price of a mistake here is measured not by loss but by the number of paths that will no longer exist.

So, other things being equal, we choose what can be reversed. And the irreversible is justified separately and strongly, otherwise it is not a decision but a bet.

Popper said the same about social reconstruction: gradual is better than all at once. Not out of cowardice, but because a large reconstruction is irreversible and therefore is tested only once, on living people.

### Not knowing is neither zero nor a dream

What we do not know, we do not know. This is not "there is nothing there" and not "it is definitely fine there": the unknown says nothing, and trying to read an answer into it substitutes one's own wish for a fact.

Hence two things that sound boring and decide almost everything. Nobody profits from the unknown. Nobody writes off what is unfamiliar.

And if a decision depends on the unknown, find out first. Not "decide and see" but find out: the difference is in who pays for the check.

Here belongs a rule the standard states separately: not knowing someone's motives gives no one the right to declare his future hopeless.

### Freedom is not a pie

You do not have to take freedom away to give it to another. Something new, whether a medicine, a road, a tool or working connectivity, increases the possibilities of all at once without making anyone poorer.

So the question is not "whom to take from" but "what can be created without taking from anyone". And the second half of this rule matters no less: what counts is not paths but that they work. Ten roads into a dead end are not freedom yet. One road to people is freedom.

A caveat is needed here, so that the point does not become naive. Growth is not guaranteed: it is rare, it takes work and usually it takes time. "Creation instead of redistribution" is a requirement on a decision, not a promise that everyone will always have enough.

### The incomprehensible is not an enemy

If we do not understand someone, that does not mean he is bad or that he can be written off. Until we have made sense of him, the only argument against him comes down to our unfamiliarity with him. That is no argument. It is a hole in our knowledge, not a property of the person.

The incomprehensible is a question, not a verdict. And from this comes what makes this whole measure alive, not merely correct: a verdict about someone's future is not an opinion but a fact that can be checked. Until it is checked, no one has the right to cancel someone else's future.

## Part VI. Cross-check and boundaries

### This is not a new morality but a refinement of the old

Let us go through the correspondences, because the answer to "is this moral" lies in them.

Not sacrificing a human being: the golden rule and Kant. The irreversible: "do no harm" and ordinary caution. The worst case instead of the average: Rawls's maximin. Well-being as real possibilities, not as a feeling: Sen's capability approach. Taking the destroyer out of the game is the right of self-defence. Honesty in not knowing rests on the general ban on profiting from another's ignorance.

But there are three places where this measure is stricter than ordinary morality, and they must be stated plainly.

It does not weigh losses against gains. Usually we are ready to sacrifice a little for a lot; here there is no such accounting at all.

It does not accept consent as justification. Saying "he agreed himself" does not work.

It does not permit a small evil for a great good. And that permission we grant ourselves constantly.

This is not amorality. It is a refusal of the arithmetic that permits sacrifice in the first place.

### What this measure does not promise

I will say it plainly, because promising too much means letting the reader down.

It does not promise well-being to everyone at the snap of fingers. It does not promise an end to conflicts. It does not answer the question "what exactly to build": the index does not say what to do, it says what is inadmissible, and thereby narrows the field of choice.

The guarantee here is one, narrow and checkable: an option that destroys someone will not be chosen, provided that people count by the rules and show the accounting.

And one more thing it does not do: it does not decide for a person what matters to him. It counts paths, it does not point which one to take.

## Part VII. Objections and verification

### Six objections and answers

"But what if a person himself wants less?" A reduction in paths is a loss, even if the person chose it. But you cannot make someone happy by force either: the measure does not permit it, and for the same reason.

"Who counts the paths?" Whoever makes the decision. But he is obliged to show the accounting. Not knowing is not counted as zero here, otherwise "I did not count" would become a way to circumvent the rule.

"There are many paths, but all are bad." A path that leads nowhere is not a path. Quality enters the accounting, otherwise the measure would count formal permissions.

"Why not take preferences into account?" Because preferences can be cultivated, including into slavish ones. This was discussed above, and it is the most vulnerable place in the whole construction. If you have a stronger argument, I will listen to it.

"And if the formula gives an unexpected answer?" That is what it is for. If a conclusion contradicts a wish, that is a test, not an error, provided the calculation can be repeated and contested.

"This is a utopia, how would it be introduced?" The measure does not require a world government. It is applied by anyone who makes decisions and is obliged to explain them: an engineer, a doctor, an official, a designer.

### How to make sure this is not invented

I am not asking you to believe. There is little to check here, because the whole article has exactly two input statements.

The first: the goal is the well-being of all, and nobody is put outside the brackets. The second: a person's well-being is the number of paths he really has left. Everything else in the article is obliged to derive from these two sentences: why the sum, the average and the majority do not fit, and also why the loss of one is not paid for by someone else's gain. If somewhere I smuggled in a third input silently, that is the place where I can be caught.

The mathematics here is not mine. John Nash proved in 1950 that if an agreement must not depend on how utilities are written down, must not let one side press the other, and cannot be improved for anyone without worsening someone else, then the only such record is the product of quantities, not their sum. This is not an aphorism but a theorem with a proof, printed in Econometrica, and people read it in full. From it also follows that the loss of one is not outweighed by another's gain: a factor that reached zero nulls the whole product, however large the others.

The caveats mentioned above add nothing to this. They do not change the conclusion, they only let you compute it: substitute a small constant for zero, fix the set of the counted, do not admit to comparison what closes recovery, take the worst of the plausible cases. I named each of them aloud, and each can be contested separately without touching the theorem.

That is the whole argument: two input statements, one proven theorem and four named caveats. Everything I added myself is marked in the text as mine, not passed off as someone else's norm. It can be checked step by step: take any consequence from Parts V and VI, write down what it follows from, and see whether it still holds if you remove even one input. Where it still holds, I have overclaimed. And if you find such a place, write to me and I will check it. That, honestly, is the only promise I can make in place of the Sphere from "Picnic".
